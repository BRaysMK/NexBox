package com.nexbox.app.ui.screen

import android.os.Build
import android.view.WindowManager
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.ChevronRight
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.MusicNote
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.SkipNext
import androidx.compose.material.icons.rounded.SkipPrevious
import androidx.compose.material.icons.rounded.VolumeUp
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.window.DialogWindowProvider
import coil3.compose.AsyncImage
import coil3.request.ImageRequest
import coil3.request.crossfade
import com.nexbox.app.data.LyricsLine
import com.nexbox.app.data.MusicState
import com.nexbox.app.ui.AppCard
import com.nexbox.app.ui.ElasticCircleButton
import com.nexbox.app.ui.consumeDialogClick
import com.nexbox.app.ui.dialogScrimClick
import com.nexbox.app.ui.elasticPress
import com.nexbox.app.ui.theme.Accent
import com.nexbox.app.ui.theme.FillColor
import com.nexbox.app.ui.theme.Hairline
import com.nexbox.app.ui.theme.TextSecondary
import kotlinx.coroutines.delay
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.sin

/**
 * 主页「电脑音乐」小控件 + 完整遥控弹窗。
 *
 * mini 卡连接 PC 后才显示在文件互传卡上方（未连接时不显示，歌名处显示
 * 「PC端暂无音乐播放」，控制钮同样在但置灰）；已在播时卡上直接播放/暂停、上/下一曲。
 * 整卡点击弹出**固定大小**的完整遥控：底部固定进度条 + 三控制钮，中间有歌词就滚动
 * 高亮、没有就显示律动条。
 * 状态全部来自 [MusicRemoteViewModel] 的 WS 帧驱动，这里不做任何轮询。
 */

// ───────────────────────── 主页 mini 控件 ─────────────────────────

@Composable
fun MusicMiniCard(
    state: MusicRemoteUiState,
    onOpen: () -> Unit,
    onPlayPause: () -> Unit,
    onNext: () -> Unit,
    onPrev: () -> Unit,
) {
    // 非空即「正在播一首歌」：connected + 标题非空（PC 端同口径），后续 smart cast 全靠它
    val music = state.music?.takeIf { state.connected && it.title.isNotBlank() }

    AppCard(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(16.dp))
            .clickable(onClick = onOpen),
        shape = RoundedCornerShape(16.dp),
    ) {
        Column {
            Row(
                Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                // 封面 / 占位：无歌时也是同样的布局结构
                Box(
                    Modifier
                        .size(36.dp)
                        .clip(RoundedCornerShape(12.dp))
                        .background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.35f)),
                        contentAlignment = Alignment.Center,
                ) {
                    if (music != null && state.coverUrl != null) {
                        AsyncImage(
                            model = ImageRequest.Builder(LocalContext.current)
                                .data(state.coverUrl).crossfade(240).build(),
                            contentDescription = null,
                            contentScale = ContentScale.Crop,
                            modifier = Modifier.fillMaxSize(),
                        )
                    } else {
                        Icon(
                            Icons.Rounded.MusicNote,
                            contentDescription = null,
                            tint = Accent,
                            modifier = Modifier.size(20.dp),
                        )
                    }
                }
                Spacer(Modifier.width(12.dp))

                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Text(
                        if (music != null) music.title else "PC端暂无音乐播放",
                        fontSize = 14.sp,
                        fontWeight = FontWeight.SemiBold,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        color = if (music != null) {
                            MaterialTheme.colorScheme.onSurface
                        } else {
                            TextSecondary
                        },
                    )
                    Text(
                        if (music != null) {
                            listOfNotNull(
                                music.artist.takeIf { it.isNotBlank() },
                                if (music.playing) "播放中" else "已暂停",
                            ).joinToString(" · ")
                        } else if (state.connected) {
                            "在电脑上打开播放器后会自动出现"
                        } else {
                            "连接 PC 后可在这里遥控电脑播放"
                        },
                        fontSize = 11.sp,
                        color = TextSecondary,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }

                // 三个控制钮恒在：无歌时置灰禁用，保持布局不跳
                MiniControlButton(
                    icon = Icons.Rounded.SkipPrevious,
                    desc = "上一曲",
                    filled = false,
                    enabled = music != null,
                    onClick = onPrev,
                )
                Spacer(Modifier.width(6.dp))
                MiniControlButton(
                    icon = if (music != null && music.playing) Icons.Rounded.Pause else Icons.Rounded.PlayArrow,
                    desc = if (music != null && music.playing) "暂停" else "播放",
                    filled = true,
                    enabled = music != null,
                    onClick = onPlayPause,
                )
                Spacer(Modifier.width(6.dp))
                MiniControlButton(
                    icon = Icons.Rounded.SkipNext,
                    desc = "下一曲",
                    filled = false,
                    enabled = music != null,
                    onClick = onNext,
                )
            }

            // 底部细进度线：播放器身份感 + 一眼看到走到哪了
            if (music != null && music.durationMs > 0) {
                MiniProgressBar(music)
            }
        }
    }
}

/** mini 卡上的圆形小控制钮：播放/暂停实底，上/下一曲描边；[enabled]=false 时整钮置灰不可点 */
@Composable
private fun MiniControlButton(
    icon: ImageVector,
    desc: String,
    filled: Boolean,
    enabled: Boolean,
    onClick: () -> Unit,
) {
    ElasticCircleButton(
        icon = icon,
        contentDescription = desc,
        onClick = onClick,
        enabled = enabled,
        diameter = 36.dp,
        iconSize = if (filled) 20.dp else 22.dp,
        containerColor = if (filled) Accent else FillColor,
        iconTintColor = if (filled) Color.White else MaterialTheme.colorScheme.onSurface,
        borderColor = if (filled) Color.Transparent else Hairline,
        disabledAlpha = 0.35f,
    )
}

/** mini 卡底部 2.5dp 进度线；位置本机推算（400ms 一拍），不额外发请求 */
@Composable
private fun MiniProgressBar(music: MusicState) {
    var nowMs by remember { mutableLongStateOf(System.currentTimeMillis()) }
    LaunchedEffect(Unit) {
        while (true) {
            nowMs = System.currentTimeMillis()
            delay(400)
        }
    }
    val pos = music.displayPositionMs(nowMs)
    Box(
        Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp)
            .height(2.5.dp)
            .clip(RoundedCornerShape(2.dp))
            .background(FillColor),
    ) {
        Box(
            Modifier
                .fillMaxWidth(pos.toFloat() / music.durationMs)
                .fillMaxHeight()
                .clip(RoundedCornerShape(2.dp))
                .background(Accent),
        )
    }
    Spacer(Modifier.height(10.dp))
}

// ───────────────────────── 完整遥控弹窗 ─────────────────────────

/**
 * 完整遥控弹窗：**固定大小**（高 520dp 不随内容跳变）。
 * 布局三段：头部（标题+关闭）/ 中部（封面+歌曲信息+歌词或律动条，weight 弹性）/
 * 底部固定（进度条 + 上一曲/暂停/下一曲居中，右下角音量钮打开**独立音量弹窗**）。
 */
@Composable
fun MusicRemoteDialog(
    state: MusicRemoteUiState,
    onDismiss: () -> Unit,
    onPlayPause: () -> Unit,
    onNext: () -> Unit,
    onPrev: () -> Unit,
    onSeek: (Long) -> Unit,
    /** 打开音量弹窗时回调（PC 端拉取当前音量供滑杆初值） */
    onVolumeDialogShown: () -> Unit,
    onPcVolumeChange: (Int, Boolean) -> Unit,
) {
    // 音量弹窗开关：独立小弹窗，不占主弹窗布局
    var showVolumeDialog by remember { mutableStateOf(false) }
    // 弹窗窗口背景模糊（Android 12+ 的 blur behind）；设备关闭系统模糊开关时退回暗色遮罩
    val view = LocalView.current
    SideEffect {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            (view.parent as? DialogWindowProvider)?.window?.let { window ->
                window.addFlags(WindowManager.LayoutParams.FLAG_BLUR_BEHIND)
                window.attributes = window.attributes.apply {
                    blurBehindRadius = (22 * view.resources.displayMetrics.density).toInt()
                }
                window.setDimAmount(0.32f)
            }
        }
    }

    // 非空即「正在播一首歌」：connected + 标题非空（PC 端同口径）
    val music = state.music?.takeIf { state.connected && it.title.isNotBlank() }

    Dialog(
        onDismissRequest = onDismiss,
        properties = DialogProperties(usePlatformDefaultWidth = false),
    ) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(horizontal = 18.dp)
                // 内容铺满全屏导致系统"点外部关闭"永远不触发：点空白手动关闭
                .dialogScrimClick(onDismiss),
            contentAlignment = Alignment.Center,
        ) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(520.dp)
                    .clip(RoundedCornerShape(22.dp))
                    .background(MaterialTheme.colorScheme.surface)
                    // 卡片消费点击：点卡片本体不关弹窗（内部按钮/滑杆各自优先消费）
                    .consumeDialogClick()
                    .padding(18.dp),
            ) {
                // ── 头部：标题 + 关闭 ──
                Row(
                    Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(Modifier.weight(1f)) {
                        Text(
                            "电脑音乐",
                            fontSize = 16.sp,
                            fontWeight = FontWeight.SemiBold,
                            color = MaterialTheme.colorScheme.onSurface,
                        )
                        Text(
                            when {
                                !state.connected -> "未连接 PC 端"
                                music != null -> if (music.playing) "播放中" else "已暂停"
                                else -> "PC端暂无音乐播放"
                            },
                            fontSize = 11.sp,
                            color = TextSecondary,
                        )
                    }
                    Box(
                        Modifier
                            .size(32.dp)
                            .elasticPress(onClick = onDismiss)
                            .clip(CircleShape)
                            .background(MaterialTheme.colorScheme.surface.copy(alpha = 0.72f)),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(
                            Icons.Rounded.Close,
                            contentDescription = "关闭",
                            tint = TextSecondary,
                            modifier = Modifier.size(20.dp),
                        )
                    }
                }

                // ── 中部：封面 + 信息 + 歌词/律动（weight 弹性吃满剩余高度）──
                BoxWithMiddleContent(
                    modifier = Modifier
                        .weight(1f)
                        .fillMaxWidth(),
                    state = state,
                    music = music,
                )

                // ── 底部固定：进度条 + 中间三钮 / 右下角音量 ──
                PlayerBottomBar(
                    music = music,
                    connected = state.connected,
                    onPlayPause = onPlayPause,
                    onNext = onNext,
                    onPrev = onPrev,
                    onSeek = onSeek,
                    onShowVolumeDialog = {
                        showVolumeDialog = true
                        onVolumeDialogShown()
                    },
                )

                // 弹窗窗口不消费手势区，留一点底部余量避免贴住导航条
                Spacer(Modifier.navigationBarsPadding().height(0.dp))
            }
        }
    }

    // 独立音量弹窗：主弹窗之上再开一个小窗口
    if (showVolumeDialog) {
        VolumeDialog(
            pcVolume = state.pcVolume,
            onPcVolumeChange = onPcVolumeChange,
            onDismiss = { showVolumeDialog = false },
        )
    }
}

/** 弹窗中部：封面 + 标题/歌手/来源 + 歌词（有则滚动高亮，无则律动条） */
@Composable
private fun BoxWithMiddleContent(
    modifier: Modifier = Modifier,
    state: MusicRemoteUiState,
    music: MusicState?,
) {
    val hasContent = music != null
    Column(
        modifier = modifier,
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Spacer(Modifier.height(10.dp))

        // ── 封面 ──
        Box(
            Modifier
                .size(116.dp)
                .clip(RoundedCornerShape(16.dp))
                .background(FillColor)
                .border(1.dp, Hairline, RoundedCornerShape(16.dp)),
            contentAlignment = Alignment.Center,
        ) {
            if (music != null && state.coverUrl != null) {
                AsyncImage(
                    // URL 内容寻址（coverKey 不变则 URL 不变），Coil 自带内存/磁盘缓存
                    model = ImageRequest.Builder(LocalContext.current)
                        .data(state.coverUrl).crossfade(240).build(),
                    contentDescription = "封面",
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.fillMaxSize(),
                )
            } else {
                Icon(
                    Icons.Rounded.MusicNote,
                    contentDescription = null,
                    tint = TextSecondary.copy(alpha = 0.55f),
                    modifier = Modifier.size(44.dp),
                )
            }
        }

        Spacer(Modifier.height(10.dp))

        // ── 标题 / 歌手 ──
        Text(
            music?.title?.ifBlank { null } ?: "PC端暂无音乐播放",
            fontSize = 16.sp,
            fontWeight = FontWeight.SemiBold,
            textAlign = TextAlign.Center,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            color = if (hasContent) MaterialTheme.colorScheme.onSurface else TextSecondary,
        )
        val subtitle = music?.let {
            listOfNotNull(
                it.artist.takeIf { a -> a.isNotBlank() },
                it.album.takeIf { a -> a.isNotBlank() },
            ).joinToString(" — ")
        }
        if (!subtitle.isNullOrBlank()) {
            Spacer(Modifier.height(3.dp))
            Text(
                subtitle,
                fontSize = 11.5.sp,
                color = TextSecondary,
                textAlign = TextAlign.Center,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }

        Spacer(Modifier.height(6.dp))

        // ── 歌词 / 律动：吃掉剩余高度 ──
        // 有进度 → 只显示歌词，没歌词就留白（无「暂无歌词」字样、无播放源标签、无律动条）；
        // 获取不到进度（无播放 / 没有时长信息）→ 只显示律动条
        val lyrics = state.lyrics
        val lyricLines = lyrics?.lines.orEmpty()
        when {
            music != null && music.durationMs > 0 && lyricLines.isNotEmpty() -> LyricsScroller(
                lines = lyricLines,
                music = music,
                modifier = Modifier
                    .weight(1f)
                    .fillMaxWidth()
                    .padding(top = 10.dp),
            )

            music != null -> Spacer(
                Modifier
                    .weight(1f)
                    .fillMaxWidth(),
            )

            else -> GrooveBars(
                // 无进度但在播（个别播放器不报时长）：律动条摆起来；无播放则静止
                playing = music?.playing == true,
                modifier = Modifier
                    .weight(1f)
                    .fillMaxWidth()
                    .padding(top = 10.dp),
            )
        }
    }
}

/**
 * 滚动歌词：按本机推算的实时进度高亮当前行并自动滚动（当前行大致停在偏上 1/3 处）。
 * 进度与外层共享同一时钟节拍（400ms），高亮行切换时机与真实歌词误差在半秒内。
 */
@Composable
private fun LyricsScroller(
    lines: List<LyricsLine>,
    music: MusicState,
    modifier: Modifier = Modifier,
) {
    var nowMs by remember { mutableLongStateOf(System.currentTimeMillis()) }
    LaunchedEffect(Unit) {
        while (true) {
            nowMs = System.currentTimeMillis()
            delay(400)
        }
    }
    val positionMs = music.displayPositionMs(nowMs)
    val activeIndex = lines.indexOfLast { it.timeMs <= positionMs }

    val listState = rememberLazyListState()
    LaunchedEffect(activeIndex) {
        // 当前行滚到偏上位置（留出上文两行），到顶就不滚
        val target = (activeIndex - 2).coerceAtLeast(0)
        if (target >= 0) listState.animateScrollToItem(target)
    }

    LazyColumn(
        state = listState,
        modifier = modifier,
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(9.dp),
    ) {
        itemsIndexed(lines) { index, line ->
            val isActive = index == activeIndex
            Text(
                line.text,
                fontSize = if (isActive) 13.sp else 12.sp,
                lineHeight = if (isActive) 18.sp else 16.sp,
                fontWeight = if (isActive) FontWeight.SemiBold else FontWeight.Normal,
                color = if (isActive) Accent else TextSecondary.copy(alpha = 0.62f),
                textAlign = TextAlign.Center,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}

/**
 * 律动条：五根竖条随播放相位摆动；暂停时平滑收回到静止矮条。
 * 只在「获取不到歌曲进度」时出现（弹窗中部 / 底栏进度条位置），[barMaxHeight] 控制条高。
 */
@Composable
private fun GrooveBars(playing: Boolean, modifier: Modifier = Modifier, barMaxHeight: Int = 30) {
    val transition = rememberInfiniteTransition(label = "groove")
    val phase by transition.animateFloat(
        initialValue = 0f,
        targetValue = (2f * PI).toFloat(),
        infiniteRepeatable(tween(1600, easing = LinearEasing)),
        label = "phase",
    )
    // 振幅随播放状态平滑过渡：暂停收、播放起，不会突跳
    val amplitude by animateFloatAsState(if (playing) 1f else 0f, label = "amplitude")

    Row(
        modifier = modifier,
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(7.dp, Alignment.CenterHorizontally),
    ) {
        repeat(5) { i ->
            // 各条相位错开；基础高度也各异，摆起来更像律动而不是复制粘贴
            val base = 0.30f + 0.10f * ((i * 2 + 1) % 3)
            val wobble = if (amplitude > 0f) {
                (sin(phase + i * 1.25f).let { abs(it) * 0.5f + 0.5f }) * 0.55f * amplitude
            } else {
                0f
            }
            Box(
                Modifier
                    .width(5.dp)
                    .height((barMaxHeight * (base + wobble)).dp)
                    .clip(RoundedCornerShape(3.dp))
                    .background(Accent.copy(alpha = 0.28f + 0.5f * ((base + wobble - 0.3f).coerceIn(0f, 1f)))),
            )
        }
    }
}

/**
 * 弹窗底部固定控制栏：进度条 + 时间 + 控制排（中间上一曲/暂停/下一曲居中，右下角音量钮）。
 * [music] 为 null（无播放）时按钮与进度条整体禁用但占位不变，布局不跳。
 */
@Composable
private fun PlayerBottomBar(
    music: MusicState?,
    connected: Boolean,
    onPlayPause: () -> Unit,
    onNext: () -> Unit,
    onPrev: () -> Unit,
    onSeek: (Long) -> Unit,
    onShowVolumeDialog: () -> Unit,
) {
    val hasContent = music != null
    val duration = music?.durationMs?.coerceAtLeast(0) ?: 0L

    // 拖动中的临时值（0..1）。松手发 seek 后保留到 seek 生效帧到达再对齐：
    // 拖动中帧到了也不能清（会打断手指）；松手后 PC 端生效要几百毫秒，
    // 中间帧还是旧位置，直接清会先回跳再跳目标 —— 表现为「闪一下」
    var dragFraction by remember { mutableFloatStateOf(Float.NaN) }
    var dragging by remember { mutableStateOf(false) }
    // seek 已发出但 PC 端还没生效的目标位置（-1 = 无 pending seek）
    var pendingSeekMs by remember { mutableLongStateOf(-1L) }
    var pendingSeekAt by remember { mutableLongStateOf(0L) }
    var nowMs by remember { mutableLongStateOf(System.currentTimeMillis()) }

    // PC 端每帧都会刷新 updatedAt，把它当帧序号用：非拖动状态下收到新帧即丢弃本地残留。
    // 有 pending seek 时只认「位置已贴近目标」的帧（seek 生效）或超时帧，中间旧位置帧忽略
    LaunchedEffect(music?.updatedAt) {
        if (!dragging && !dragFraction.isNaN()) {
            val target = pendingSeekMs
            if (target < 0) {
                dragFraction = Float.NaN
            } else {
                val framePos = music?.positionMs ?: 0L
                val waited = System.currentTimeMillis() - pendingSeekAt
                if (abs(framePos - target) <= 2500 || waited > 3000) {
                    dragFraction = Float.NaN
                    pendingSeekMs = -1L
                }
            }
        }
    }
    LaunchedEffect(Unit) {
        while (true) {
            nowMs = System.currentTimeMillis()
            delay(400)
        }
    }

    val displayPosMs = when {
        music == null -> 0L
        !dragFraction.isNaN() -> (dragFraction * duration).toLong()
        else -> music.displayPositionMs(nowMs)
    }

    Spacer(Modifier.height(8.dp))

    Column(Modifier.fillMaxWidth()) {
        if (duration > 0) {
            Slider(
                value = if (dragFraction.isNaN()) displayPosMs.toFloat() / duration else dragFraction,
                onValueChange = {
                    dragFraction = it
                    dragging = true
                },
                onValueChangeFinished = {
                    dragging = false
                    if (!dragFraction.isNaN()) {
                        val target = (dragFraction * duration).toLong()
                        onSeek(target)
                        pendingSeekMs = target
                        pendingSeekAt = System.currentTimeMillis()
                    }
                },
                // PC 端对 QQ 音乐这类来源会下发 canSeek=false，进度条直接不可拖
                enabled = music != null && music.canSeek,
                colors = SliderDefaults.colors(
                    thumbColor = Accent,
                    activeTrackColor = Accent,
                    inactiveTrackColor = FillColor,
                ),
                modifier = Modifier.fillMaxWidth(),
            )
            Row(
                Modifier
                    .fillMaxWidth()
                    .padding(top = 0.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Text(fmtTime(displayPosMs), fontSize = 11.sp, color = TextSecondary)
                Text(fmtTime(duration), fontSize = 11.sp, color = TextSecondary)
            }
        } else {
            // 获取不到歌曲进度：在播时进度条位置显示律动条（随播放摆动）；
            // 未播放时静态等高占位——中部已经有一组静止律动条，这里再来一组会重复
            if (music?.playing == true) {
                GrooveBars(
                    playing = true,
                    barMaxHeight = 16,
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(18.dp),
                )
                Spacer(Modifier.height(6.dp))
            } else {
                Spacer(Modifier.height(24.dp))
            }
        }

        Spacer(Modifier.height(6.dp))

        // ── 控制排：中间三钮居中，右下角音量钮 ──
        Box(Modifier.fillMaxWidth()) {
            Row(
                Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(24.dp, Alignment.CenterHorizontally),
            ) {
                IconButtonGhost(Icons.Rounded.SkipPrevious, "上一曲", enabled = hasContent, onClick = onPrev)
                ElasticCircleButton(
                    icon = if (music != null && music.playing) Icons.Rounded.Pause else Icons.Rounded.PlayArrow,
                    contentDescription = if (music != null && music.playing) "暂停" else "播放",
                    onClick = onPlayPause,
                    enabled = hasContent,
                    diameter = 58.dp,
                    iconSize = 30.dp,
                    containerColor = Accent,
                    iconTintColor = Color.White,
                    disabledAlpha = 0.4f,
                )
                IconButtonGhost(Icons.Rounded.SkipNext, "下一曲", enabled = hasContent, onClick = onNext)
            }

            // 右下角：音量调节（打开独立音量弹窗）
            BottomCornerButton(
                icon = Icons.Rounded.VolumeUp,
                desc = "音量调节",
                active = false,
                enabled = connected,
                onClick = onShowVolumeDialog,
                modifier = Modifier.align(Alignment.CenterEnd),
            )
        }
    }
}

/** 底部角上的圆形功能钮：激活时品牌色描边填充 */
@Composable
private fun BottomCornerButton(
    icon: ImageVector,
    desc: String,
    active: Boolean,
    enabled: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    ElasticCircleButton(
        icon = icon,
        contentDescription = desc,
        onClick = onClick,
        modifier = modifier,
        enabled = enabled,
        diameter = 46.dp,
        iconSize = 22.dp,
        containerColor = if (active) Accent.copy(alpha = 0.18f) else FillColor,
        iconTintColor = if (active) Accent else MaterialTheme.colorScheme.onSurface,
        borderColor = if (active) Accent.copy(alpha = 0.65f) else Hairline,
        disabledAlpha = 0.4f,
    )
}

/** 独立音量弹窗：只调 PC 系统主音量 */
@Composable
private fun VolumeDialog(
    pcVolume: Int?,
    onPcVolumeChange: (Int, Boolean) -> Unit,
    onDismiss: () -> Unit,
) {
    Dialog(onDismissRequest = onDismiss) {
        Column(
            Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(18.dp))
                .background(MaterialTheme.colorScheme.surface)
                .border(1.dp, Hairline, RoundedCornerShape(18.dp))
                .padding(horizontal = 20.dp, vertical = 16.dp),
        ) {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                Text(
                    "电脑音量",
                    fontSize = 13.sp,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                Text(pcVolume?.let { "$it%" } ?: "--", fontSize = 13.sp, color = TextSecondary)
            }
            Spacer(Modifier.height(4.dp))
            // 拖动/点击期间滑杆由本地值驱动：onValueChangeFinished 闭包捕获的
            // pcVolume 是旧重组的值，tap 场景下会把 onValueChange 刚发的新值又写回去
            // （表现为"点了没反应"）。松手用本地最新值强制发送，与进度条同款模式。
            var dragValue by remember { mutableStateOf<Float?>(null) }
            Slider(
                value = dragValue ?: (pcVolume ?: 0) / 100f,
                onValueChange = {
                    dragValue = it
                    onPcVolumeChange((it * 100).toInt(), false)
                },
                onValueChangeFinished = {
                    dragValue?.let { onPcVolumeChange((it * 100).toInt(), true) }
                    dragValue = null
                },
                colors = SliderDefaults.colors(
                    thumbColor = Accent,
                    activeTrackColor = Accent,
                    inactiveTrackColor = FillColor,
                ),
                modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}

@Composable
private fun IconButtonGhost(icon: ImageVector, contentDescription: String, enabled: Boolean, onClick: () -> Unit) {
    ElasticCircleButton(
        icon = icon,
        contentDescription = contentDescription,
        onClick = onClick,
        enabled = enabled,
        diameter = 50.dp,
        iconSize = 32.dp,
        containerColor = Color.Transparent,
        iconTintColor = MaterialTheme.colorScheme.onSurface,
        disabledAlpha = 0.4f,
    )
}

/** 毫秒 → m:ss（音乐时长不超过一小时，够用） */
private fun fmtTime(ms: Long): String {
    val total = (ms.coerceAtLeast(0)) / 1000
    return "%d:%02d".format(total / 60, total % 60)
}

/**
 * 本机推算的实时播放位置：帧位置 + 本地流逝时间。
 * 时钟偏差保护：手机与 PC 的 epoch 时钟可能差几秒，elapsed 落在 0..10s 之外
 * 就退回直接用 PC 报的位置（不推算），避免歌曲刚开始时进度乱跳/闪。
 */
internal fun MusicState.displayPositionMs(nowMs: Long): Long {
    val base = positionMs.coerceIn(0L, durationMs)
    if (!playing || durationMs <= 0) return base
    val elapsed = nowMs - updatedAt
    if (elapsed < 0 || elapsed > 10_000) return base
    return (base + elapsed).coerceIn(0L, durationMs)
}
