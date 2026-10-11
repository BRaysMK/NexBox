package com.nexbox.app.data

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * 剪贴板同步管理器（PC ↔ 本机 · 纯文本）。
 *
 * 方向与开关语义（与 PC 端对称）：
 * - 发送（本机 → PC）：App 前台 + 开关开时，每 1.5s 轮询系统剪贴板，
 *   内容指纹（FNV-1a）变化才 POST /api/clipboard 推给 PC。开关只管这个方向。
 * - 接收（PC → 本机）：App 前台时订阅 WS 事件，收到 `clipboard.update` 帧写系统剪贴板。
 *   始终开启，不受开关影响。
 *
 * Android 10+ 限制：应用没有窗口焦点时读不到剪贴板（getPrimaryClip 返回 null），
 * 后台写入同样受限，所以同步只在 App 前台期间工作；退后台自动暂停全部协程。
 *
 * 回前台补齐（PC → 本机方向的后台缺口）：切前台时若**本机剪贴板自上次之后没有变化**
 * （指纹仍等于 lastHash），才拉一次 PC 当前剪贴板覆盖本地；若本地已有新复制的内容，
 * 视本地为更新，跳过拉取、交给轮询推给 PC——保证「手机复制 → 打开 App → PC 能粘贴」
 * 这个主场景不被后台补齐覆盖。
 *
 * 回环抑制与 PC 端对称：lastHash 记录最近一次已知内容指纹，
 * 「收到 PC 内容写本机」与「本机内容推 PC」两条路径都更新它，避免 A→B→A 死循环。
 */
object ClipboardSyncManager {

    /** 与 PC 端一致的单段文本上限（UTF-8 字节） */
    private const val MAX_TEXT_BYTES = 512 * 1024

    /** 发送轮询间隔。刻意比 PC 端（1s）长：读剪贴板会触发 Android 12+ 的「已粘贴」系统提示，放慢一档降低出现频率 */
    private const val POLL_INTERVAL_MS = 1_500L

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    private var appContext: Context? = null

    @Volatile
    private var foreground = false

    /** 最近一次已知剪贴板内容指纹（回环抑制 + 补齐判断） */
    @Volatile
    private var lastHash = 0L

    private var pollJob: Job? = null
    private var receiveJob: Job? = null

    fun init(context: Context) {
        appContext = context.applicationContext
    }

    /** MainActivity.onStart：进前台，按当前状态拉起协程，并尝试补齐后台错过的 PC 端变化 */
    fun onAppForeground() {
        foreground = true
        syncJobs()
        scope.launch { catchUpFromPc() }
    }

    /** MainActivity.onStop：退后台，全部暂停（Android 10+ 无焦点读写剪贴板都受限） */
    fun onAppBackground() {
        foreground = false
        syncJobs()
    }

    /** 开关变化（ClipboardSyncStore.setEnabled 调用）：只影响发送轮询 */
    fun onEnabledChanged() {
        syncJobs()
    }

    /** 按当前条件启停两条协程（幂等） */
    private fun syncJobs() {
        val ctx = appContext ?: return
        // 接收方向：前台即启用（开关无关）
        if (foreground) {
            if (receiveJob?.isActive != true) {
                receiveJob = scope.launch { receiveLoop() }
            }
        } else {
            receiveJob?.cancel()
            receiveJob = null
        }
        // 发送方向：前台 + 开关开
        if (foreground && ClipboardSyncStore.enabled) {
            if (pollJob?.isActive != true) {
                pollJob = scope.launch { pollLoop(ctx) }
            }
        } else {
            pollJob?.cancel()
            pollJob = null
        }
    }

    /**
     * 回前台补齐：仅当本机剪贴板自上次已知后没有变化时，才拉 PC 当前内容覆盖本地。
     * 本地有新内容时跳过（本地为更新者，轮询会把它推给 PC）。
     */
    private suspend fun catchUpFromPc() {
        val ctx = appContext ?: return
        // 本地剪贴板与最后已知指纹不一致 = 用户新复制了内容 → 本地优先，不拉 PC
        val local = readLocalClipboard(ctx)
        if (local != null && fnv1a64(local) != lastHash) return

        val remote = runCatching { NetworkModule.api.clipboardPull() }.getOrNull()
            ?.takeIf { it.isNotEmpty() && it.toByteArray(Charsets.UTF_8).size <= MAX_TEXT_BYTES } ?: return
        val h = fnv1a64(remote)
        if (h == lastHash) return
        lastHash = h
        writeSystemClipboard(ctx, remote)
    }

    /** 发送轮询：读系统剪贴板 → 指纹变化 → 推给 PC。未连接时跳过且不更新指纹（重连后能补推） */
    private suspend fun pollLoop(ctx: Context) {
        while (currentCoroutineContext().isActive) {
            delay(POLL_INTERVAL_MS)
            if (!foreground || !ClipboardSyncStore.enabled) continue
            if (!NetworkModule.statsRepo.connected.value) continue
            val text = readLocalClipboard(ctx) ?: continue
            val h = fnv1a64(text)
            if (h == lastHash) continue
            // 推送成功才记录指纹：失败（PC 剪贴板被其他程序短暂占用等）下一轮重试
            if (pushQuietly(text)) {
                lastHash = h
            }
        }
    }

    /** 推送失败返回 false（断连/超限/网络抖动），下一轮重试；取消则正常终止 */
    private suspend fun pushQuietly(text: String): Boolean {
        return try {
            NetworkModule.api.clipboardPush(text)
            true
        } catch (e: CancellationException) {
            throw e
        } catch (_: Exception) {
            false
        }
    }

    /** 接收循环：WS `clipboard.update` 帧 → 写系统剪贴板（必须主线程） */
    private suspend fun receiveLoop() {
        NetworkModule.statsRepo.events.collect { event ->
            if (event !is WsEvent.ClipboardUpdate) return@collect
            val ctx = appContext ?: return@collect
            if (event.text.isEmpty() || event.text.toByteArray(Charsets.UTF_8).size > MAX_TEXT_BYTES) {
                return@collect
            }
            lastHash = fnv1a64(event.text)
            writeSystemClipboard(ctx, event.text)
        }
    }

    /** 读系统剪贴板首项文本；空 / 超 512KB（按 UTF-8 字节，与 PC 端口径一致）/ 无焦点读不到 统一返回 null */
    private fun readLocalClipboard(ctx: Context): String? {
        val cm = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager ?: return null
        // Android 10+ 无焦点时 primaryClip 为 null，视为无内容
        val text = cm.primaryClip?.getItemAt(0)?.coerceToText(ctx)?.toString().orEmpty()
        return text.takeIf { it.isNotEmpty() && it.toByteArray(Charsets.UTF_8).size <= MAX_TEXT_BYTES }
    }

    /** 写系统剪贴板；setPrimaryClip 必须主线程，失败（系统限制/占用）静默 */
    private suspend fun writeSystemClipboard(ctx: Context, text: String) {
        withContext(Dispatchers.Main) {
            runCatching {
                val cm = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                cm.setPrimaryClip(ClipData.newPlainText("text", text))
            }
        }
    }

    /** FNV-1a 64 位指纹（与 PC 端同算法；两端各自独立成立即可，不依赖跨端一致） */
    private fun fnv1a64(s: String): Long {
        var h = -0x340d631b7bdddcdbL // 0xcbf29ce484222325
        for (b in s.toByteArray()) {
            h = h xor (b.toLong() and 0xffL)
            h *= 0x100000001b3L
        }
        return h
    }
}
