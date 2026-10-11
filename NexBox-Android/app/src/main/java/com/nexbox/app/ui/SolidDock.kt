package com.nexbox.app.ui

import android.view.HapticFeedbackConstants
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.clipPath
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.pointer.changedToUp
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import com.nexbox.app.ui.glass.Capsule
import com.nexbox.app.ui.theme.NavSolidBorder
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.launch
import kotlin.math.abs
import kotlin.math.roundToInt
import kotlin.math.sqrt

// ───────── FlClash FloatingNavigationBar 参数（lib/widgets/navigation_dock.dart 对齐）─────────
// SpringDescription.withDurationAndBounce(duration, bounce) → stiffness=(2π/duration)²、ζ=1-bounce
private const val SETTLE_STIFFNESS = 158f    // 500ms, bounce 0.32：落位带回弹
private const val SETTLE_DAMPING = 0.68f
private const val TRACK_STIFFNESS = 2742f    // 120ms, bounce 0：拖拽跟手
private const val TRACK_DAMPING = 1f
private const val LIFT_STIFFNESS = 503f      // 280ms, bounce 0.2：按下抬起
private const val LIFT_DAMPING = 0.8f

private const val JELLY_SPEED = 8f           // 滑块速度达到该档位/s 时果冻拉伸拉满
private const val JELLY_STRETCH = 0.25f
private const val PILL_PRESS_SCALE = 78f / 56f // 按下时滑块等比放大到与玻璃版同尺寸
private const val LENS_MAGNIFY = 0.12f       // 滑块压着的标签放大比例
private const val OVERDRAG = 0.35f           // 拖出两端橡皮筋限位（档位单位）
private const val PULL_LIMIT_RATIO = 7f / 32f

/**
 * 对应 FlClash 的 _Spring：持有 value/velocity 的弹簧，springTo 可在运动中逐帧重定目标
 * （保持速度连续）。半隐式欧拉 + withFrameNanos；高刚度弹簧按 16ms 子步积分防长帧发散。
 */
private class DockSpring(
    initial: Float,
    private val settleEps: Float,
    private val settleVelEps: Float,
) {
    val value = mutableFloatStateOf(initial)
    val target = mutableFloatStateOf(initial)
    val velocity = mutableFloatStateOf(0f)
    private var job: Job? = null

    fun springTo(newTarget: Float, stiffness: Float, dampingRatio: Float, scope: CoroutineScope) {
        target.floatValue = newTarget
        if (job?.isActive == true) return
        job = scope.launch {
            var x = value.floatValue
            var v = velocity.floatValue
            val c = 2f * sqrt(stiffness) * dampingRatio
            var lastNanos = 0L
            while (true) {
                val nanos = withFrameNanos { it }
                if (lastNanos == 0L) {
                    lastNanos = nanos
                    continue
                }
                var dt = (nanos - lastNanos) / 1_000_000_000f
                lastNanos = nanos
                while (dt > 0f) {
                    val h = minOf(dt, 0.016f)
                    dt -= h
                    v += (stiffness * (target.floatValue - x) - c * v) * h
                    x += v * h
                }
                val t = target.floatValue
                if (abs(x - t) < settleEps && abs(v) < settleVelEps) {
                    value.floatValue = t
                    velocity.floatValue = 0f
                    return@launch
                }
                value.floatValue = x
                velocity.floatValue = v
            }
        }
    }
}

/**
 * 关闭液态玻璃时的底部导航坞：FlClash FloatingNavigationBar 的实色移植——
 * 按下整条 bar swell + 滑块弹到所按档位、拖拽跟手 + 两端橡皮筋、滑块随速度果冻拉伸、
 * 松手落在手指所在档位 + 触感、落位 settle 弹簧带回弹、滑块扫过标签逐块染色。
 * 零模糊零折射；hover / FAB slot 为桌面特性未移植。
 */
@Composable
internal fun SolidDock(
    selectedIndex: Int,
    onSelected: (Int) -> Unit,
    tabsCount: Int,
    barColor: Color,
    pillColor: Color,
    contentColor: Color,
    accentColor: Color,
    modifier: Modifier = Modifier,
    tabContent: @Composable (index: Int, color: Color) -> Unit,
) {
    val capsule = Capsule()
    val scope = rememberCoroutineScope()
    val view = LocalView.current

    val lens = remember { DockSpring(selectedIndex.toFloat(), settleEps = 0.0005f, settleVelEps = 0.01f) }
    val lift = remember { DockSpring(0f, settleEps = 0.0005f, settleVelEps = 0.005f) }
    val stretch = remember { DockSpring(0f, settleEps = 0.01f, settleVelEps = 2f) }
    var pressing by remember { mutableStateOf(false) }

    val latestSelected by rememberUpdatedState(selectedIndex)
    val latestOnSelected by rememberUpdatedState(onSelected)

    // 外部切页（如程序化换 tab）：落位弹簧跟过去；按住期间不打架
    LaunchedEffect(Unit) {
        snapshotFlow { latestSelected }
            .drop(1)
            .collect { index ->
                if (!pressing) {
                    lens.springTo(index.toFloat(), SETTLE_STIFFNESS, SETTLE_DAMPING, scope)
                }
            }
    }

    BoxWithConstraints(
        modifier
            .height(64.dp)
            .graphicsLayer {
                // FlClash _PressTransform：按下整条 swell，拖出边界横向橡皮筋拉伸
                val swell = 1f + lift.value.floatValue *
                    minOf(0.25f, 16.dp.toPx() / maxOf(size.width, size.height))
                scaleX = swell * (1f + abs(stretch.value.floatValue) / size.width * 0.5f)
                scaleY = swell
                translationX = stretch.value.floatValue
            }
            // 不 clip：按下放大/滑块增大要能越出 bar 边界（与玻璃版一致），胶囊外形由背景画出
            .background(barColor, capsule)
            .border(1.dp, NavSolidBorder, capsule)
            .pointerInput(tabsCount) {
                awaitEachGesture {
                    val down = awaitFirstDown(requireUnconsumed = false)
                    down.consume()
                    pressing = true
                    var pressed = positionAt(down.position.x, size.width.toFloat(), tabsCount)
                        .roundToInt()
                        .coerceIn(0, tabsCount - 1)
                    lift.springTo(1f, LIFT_STIFFNESS, LIFT_DAMPING, scope)
                    lens.springTo(pressed.toFloat(), SETTLE_STIFFNESS, SETTLE_DAMPING, scope)

                    val pressX = down.position.x
                    var dragging = false

                    fun settle(commit: Boolean) {
                        lift.springTo(0f, SETTLE_STIFFNESS, SETTLE_DAMPING, scope)
                        stretch.springTo(0f, SETTLE_STIFFNESS, SETTLE_DAMPING, scope)
                        pressing = false
                        if (!commit) {
                            lens.springTo(latestSelected.toFloat(), SETTLE_STIFFNESS, SETTLE_DAMPING, scope)
                            return
                        }
                        lens.springTo(pressed.toFloat(), SETTLE_STIFFNESS, SETTLE_DAMPING, scope)
                        if (pressed != latestSelected) latestOnSelected(pressed)
                    }

                    while (true) {
                        val event = awaitPointerEvent()
                        val change = event.changes.firstOrNull { it.id == down.id }
                        if (change == null) {
                            settle(commit = false)
                            break
                        }
                        if (change.changedToUp()) {
                            // 松手永远落在手指最后压着的档位，不做 fling 速度投影：
                            // 投影会在快速拖动时多带一档，看起来就是「没滑到却跳过去了」
                            change.consume()
                            settle(commit = true)
                            break
                        }
                        if (!dragging) {
                            if (abs(change.position.x - pressX) < viewConfiguration.touchSlop) continue
                            dragging = true
                        }
                        val x = change.position.x
                        val overshoot = x - x.coerceIn(0f, size.width.toFloat())
                        stretch.springTo(
                            rubberBand(overshoot, minOf(size.width, size.height) * PULL_LIMIT_RATIO),
                            TRACK_STIFFNESS, TRACK_DAMPING, scope,
                        )
                        val pos = positionAt(x, size.width.toFloat(), tabsCount)
                        lens.springTo(pos, TRACK_STIFFNESS, TRACK_DAMPING, scope)
                        val idx = pos.roundToInt().coerceIn(0, tabsCount - 1)
                        if (idx != pressed) {
                            view.performHapticFeedback(HapticFeedbackConstants.CLOCK_TICK)
                            pressed = idx
                        }
                    }
                }
            },
        contentAlignment = Alignment.CenterStart,
    ) {
        val density = LocalDensity.current
        val extent = (maxWidth - 8.dp) / tabsCount
        val extentPx = with(density) { extent.toPx() }
        val contentHeightPx = with(density) { 56.dp.toPx() }

        // 滑块（最底层，FlClash _Lens）：果冻拉伸 + 按下增大 + 按压时面色加深
        Box(
            Modifier
                .padding(horizontal = 4.dp)
                .size(extent, 56.dp)
                .graphicsLayer {
                    val jelly =
                        (abs(lens.velocity.floatValue) / JELLY_SPEED).coerceIn(0f, 1f) * JELLY_STRETCH
                    val press = 1f + (PILL_PRESS_SCALE - 1f) * lift.value.floatValue
                    scaleX = press * (1f + jelly)
                    scaleY = press * (1f - jelly / 2f)
                    translationX = lens.value.floatValue * extentPx
                }
                .clip(capsule)
                .drawBehind {
                    drawRect(pillColor)
                    if (lift.value.floatValue > 0f) {
                        drawRect(accentColor.copy(alpha = 0.08f * lift.value.floatValue))
                    }
                },
        )

        Row(
            Modifier
                .padding(horizontal = 4.dp)
                .height(56.dp)
                .fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            repeat(tabsCount) { index ->
                SolidDockItem(
                    index = index,
                    extentPx = extentPx,
                    contentHeightPx = contentHeightPx,
                    lens = lens,
                    lift = lift,
                    contentColor = contentColor,
                    accentColor = accentColor,
                    tabContent = tabContent,
                    modifier = Modifier.weight(1f).fillMaxHeight(),
                )
            }
        }
    }
}

/** FlClash _positionAt：横坐标 → 档位坐标（可越过两端，橡皮筋限位）。 */
private fun Density.positionAt(x: Float, barWidthPx: Float, tabsCount: Int): Float {
    val extentPx = (barWidthPx - 8.dp.toPx()) / tabsCount
    val position = (x - 4.dp.toPx()) / extentPx - 0.5f
    val lastIndex = (tabsCount - 1).toFloat()
    return when {
        position < 0f -> rubberBand(position, OVERDRAG)
        position > lastIndex -> lastIndex + rubberBand(position - lastIndex, OVERDRAG)
        else -> position
    }
}

/**
 * 单个标签：底层正常色内容，上层主题色内容裁剪到滑块矩形内（FlClash _LensTint）——
 * 滑块扫过时内容逐块变主题色；滑块压着的标签随接近程度放大 [LENS_MAGNIFY]*lift。
 */
@Composable
private fun SolidDockItem(
    index: Int,
    extentPx: Float,
    contentHeightPx: Float,
    lens: DockSpring,
    lift: DockSpring,
    contentColor: Color,
    accentColor: Color,
    tabContent: @Composable (Int, Color) -> Unit,
    modifier: Modifier = Modifier,
) {
    Box(modifier.graphicsLayer {
        val emphasis = (1f - abs(lens.value.floatValue - index)).coerceIn(0f, 1f)
        val s = 1f + LENS_MAGNIFY * emphasis * lift.value.floatValue
        scaleX = s
        scaleY = s
    }, contentAlignment = Alignment.Center) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            tabContent(index, contentColor)
        }
        Box(
            Modifier
                .fillMaxSize()
                .drawWithContent content@{
                    val jelly =
                        (abs(lens.velocity.floatValue) / JELLY_SPEED).coerceIn(0f, 1f) * JELLY_STRETCH
                    val press = 1f + (PILL_PRESS_SCALE - 1f) * lift.value.floatValue
                    val width = extentPx * press * (1f + jelly)
                    val height = contentHeightPx * press * (1f - jelly / 2f)
                    val cx = (lens.value.floatValue - index + 0.5f) * extentPx
                    val path = Path()
                    path.addRoundRect(
                        RoundRect(
                            cx - width / 2f,
                            (contentHeightPx - height) / 2f,
                            cx + width / 2f,
                            (contentHeightPx + height) / 2f,
                            CornerRadius(height / 2f),
                        ),
                    )
                    clipPath(path) { this@content.drawContent() }
                },
            contentAlignment = Alignment.Center,
        ) {
            tabContent(index, accentColor)
        }
    }
}
