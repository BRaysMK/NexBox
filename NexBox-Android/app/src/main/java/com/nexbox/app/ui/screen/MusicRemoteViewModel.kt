package com.nexbox.app.ui.screen

import android.net.Uri
import androidx.compose.runtime.Immutable
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.nexbox.app.data.MusicLyrics
import com.nexbox.app.data.MusicState
import com.nexbox.app.data.NetworkModule
import com.nexbox.app.data.WsEvent
import com.nexbox.app.data.toUserMessage
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.decodeFromJsonElement
import kotlinx.serialization.json.jsonObject

/** PC 系统音量查询响应（PC 端 query audio.volume）。
 *  percent 用 Double：PC 端历史上发过 87.0 这类浮点字面量，Int 字段会直接解码失败 */
@Serializable
private data class PcVolumeResp(val percent: Double = 0.0)

@Immutable
data class MusicRemoteUiState(
    /** 已与 PC 配对（有会话）；未连接时整个控件进引导态 */
    val connected: Boolean = false,
    /** PC 端最新「当前播放」快照；null = PC 端没有可控制的播放内容 */
    val music: MusicState? = null,
    /** 封面完整 URL（已拼 host 与 token），直接交给 Coil；无封面为 null */
    val coverUrl: String? = null,
    /**
     * 当前这首歌的歌词：PC 端随 WS 主动下发（`music.lyrics` 帧，key 变化才发）。
     * null = 还没收到（中部显示律动条，仅限无进度场景）；lines 空 = 确认无歌词。
     */
    val lyrics: MusicLyrics? = null,
    /** PC 系统主音量 0..100（打开音量弹窗时查询）；null = 未知 */
    val pcVolume: Int? = null,
    val message: String? = null,
    val messageIsError: Boolean = true,
)

/**
 * 电脑音乐遥控：状态全量来自 PC 端每秒推送的 WS `music.state` 帧
 * （本机不轮询 REST），控制走 `music.play_pause / next / prev / seek` 白名单动作
 * （source 固定 auto，由 PC 端按统一优先级路由）。
 *
 * 歌词也是 PC 端随 WS 主动下发的 `music.lyrics` 帧（软件内读前端推送缓存、
 * 外部读后台搜索缓存），本机完全被动接收，无查询、无轮询。
 *
 * 播放位置不逐帧发：帧里的 `positionMs` 是 PC 端合成时刻的位置，播放中
 * 本机用 `updatedAt` 推算实时进度（UI 层每几百毫秒取一次系统时钟重算）。
 * QQ 音乐等会汇报进度但拒绝 seek 的来源由 PC 端下发 `canSeek=false`。
 */
class MusicRemoteViewModel : ViewModel() {

    private val _state = MutableStateFlow(MusicRemoteUiState())
    val state: StateFlow<MusicRemoteUiState> = _state.asStateFlow()

    init {
        // ① 会话建立 / 断开：翻 connected，断开时清掉残留的播放状态与封面
        viewModelScope.launch {
            NetworkModule.session.session
                .distinctUntilChanged()
                .collectLatest { s ->
                    _state.update {
                        it.copy(
                            connected = s != null,
                            music = if (s == null) null else it.music,
                            coverUrl = if (s == null) null else it.coverUrl,
                        )
                    }
                }
        }

        // ② WS 帧驱动。PC 端每次新建 WS 连接都会立刻推第一帧，无需再主动 REST 拉取
        viewModelScope.launch {
            NetworkModule.statsRepo.events.collect { event ->
                when (event) {
                    is WsEvent.MusicStateUpdate -> applyMusic(event.state)
                    is WsEvent.LyricsUpdate -> applyLyrics(event.lyrics)
                    else -> Unit
                }
            }
        }
    }

    fun playPause() = control("music.play_pause", JsonObject(emptyMap()))
    fun next() = control("music.next", JsonObject(emptyMap()))
    fun prev() = control("music.prev", JsonObject(emptyMap()))
    fun seek(positionMs: Long) =
        control("music.seek", JsonObject(mapOf("positionMs" to JsonPrimitive(positionMs))))

    fun consumeMessage() = _state.update { it.copy(message = null) }

    // ───────── PC 系统音量 ─────────

    /** 打开音量弹窗时查询 PC 当前音量（0..100） */
    fun refreshPcVolume() {
        viewModelScope.launch {
            runCatching {
                NetworkModule.json.decodeFromString(
                    PcVolumeResp.serializer(),
                    NetworkModule.api.query("audio.volume").toString(),
                )
            }.onSuccess { resp ->
                _state.update { it.copy(pcVolume = resp.percent.toInt().coerceIn(0, 100)) }
            }
        }
    }

    private var lastPcVolSent = 0L

    /** 设置 PC 音量：拖动中 150ms 节流，松手强制发送 */
    fun setPcVolume(percent: Int, force: Boolean = false) {
        val p = percent.coerceIn(0, 100)
        _state.update { it.copy(pcVolume = p) }
        val now = System.currentTimeMillis()
        if (!force && now - lastPcVolSent < 150) return
        lastPcVolSent = now
        viewModelScope.launch {
            runCatching {
                NetworkModule.api.action(
                    "audio.volume",
                    JsonObject(mapOf("percent" to JsonPrimitive(p))),
                )
            }
        }
    }

    // ───────── 内部实现 ─────────

    private fun applyMusic(m: MusicState) {
        viewModelScope.launch {
            val lyrics = _state.value.lyrics
            // 换歌后旧歌词立即失效：置 null 让弹窗先回律动条，等 PC 端推新的
            val staleLyrics = if (lyrics != null && lyrics.title != m.title) null else lyrics
            _state.update { it.copy(music = m, coverUrl = buildCoverUrl(m), lyrics = staleLyrics) }
        }
    }

    /** 歌词帧到达：与当前播放对齐才应用（防换歌瞬间串词） */
    private fun applyLyrics(l: MusicLyrics) {
        val current = _state.value.music ?: return
        if (current.title != l.title) return
        _state.update { it.copy(lyrics = l) }
    }

    /**
     * 封面 URL = baseUrl + PC 给的相对路径 + token。
     * 令牌是 base64（可能含 `+ / =`），放 query 必须百分号编码，
     * 否则 PC 端把 `+` 解成空格 → 令牌不匹配 → 封面永远 401（与 WS 同一个坑）。
     */
    private suspend fun buildCoverUrl(m: MusicState): String? {
        val rel = m.coverUrl?.takeIf { it.isNotBlank() } ?: return null
        if (m.coverKey.isNullOrBlank()) return null
        val s = NetworkModule.session.current() ?: return null
        val token = Uri.encode(s.token)
        return "${s.baseUrl}$rel&token=$token"
    }

    private fun control(key: String, args: JsonObject) {
        viewModelScope.launch {
            if (NetworkModule.session.current() == null) {
                _state.update { it.copy(message = "尚未连接 PC 端，请先完成配对", messageIsError = true) }
                return@launch
            }
            // source 固定 auto：按 PC 端统一优先级打给当前生效来源
            val fullArgs = JsonObject(args + mapOf("source" to JsonPrimitive("auto")))

            // 播放/暂停做乐观翻面（PC 端生效要几百毫秒，等帧回来再翻会显得点不动）；
            // seek/切歌改不了什么本地状态，不乐观
            val before = _state.value.music
            if (key == "music.play_pause" && before != null) {
                _state.update { it.copy(music = before.copy(playing = !before.playing)) }
            }

            val result = runCatching { NetworkModule.api.action(key, fullArgs) }
            result
                .onSuccess { data ->
                    // PC 端无播放内容时返回 target:"none"（HTTP 200），不是错误但要提示
                    if (data.jsonObject["target"]?.let { (it as? JsonPrimitive)?.content } == "none") {
                        _state.update {
                            it.copy(message = "电脑上当前没有播放中的音乐", messageIsError = false)
                        }
                        // 乐观翻面回滚，等下一帧对齐
                        if (key == "music.play_pause" && before != null) {
                            _state.update { it.copy(music = before) }
                        }
                    } else {
                        _state.update { it.copy(message = null) }
                    }
                }
                .onFailure { e ->
                    if (key == "music.play_pause" && before != null) {
                        _state.update { it.copy(music = before) }
                    }
                    _state.update { it.copy(message = e.toUserMessage(), messageIsError = true) }
                }
        }
    }
}
