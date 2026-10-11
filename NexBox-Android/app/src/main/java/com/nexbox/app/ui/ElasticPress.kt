package com.nexbox.app.ui

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.AnimationSpec
import androidx.compose.animation.core.spring
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Icon
import androidx.compose.runtime.Composable
import androidx.compose.runtime.MutableFloatState
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.pointer.changedToUp
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import kotlin.math.abs
import kotlin.math.min
import kotlin.math.sign
import kotlin.math.sqrt

// ───────── FlClash ElasticPress 参数（lib/widgets/navigation_dock.dart 对齐）─────────
// _pressGrowth=1/8 → 按下最多放大约 25%；小钮再受 _maxPressGrowth 限制
// _pullLimit=7/32 → 拖拽位移上限（相对按钮最短边的比例）
// _pullStretch=0.5 → 拖拽时沿拉拽方向的拉伸形变

private const val PRESS_GROWTH = 0.25f
private const val MAX_PRESS_GROWTH_PX = 16f
private const val PULL_LIMIT_RATIO = 7f / 32f
private const val PULL_STRETCH = 0.5f

// 回弹弹簧参数：stiffness k 与阻尼比 ζ（与 spring(0.62, 500) 同手感）
private const val SETTLE_STIFFNESS = 500f
private const val SETTLE_DAMPING = 0.62f

private val LiftSpring = spring<Float>(dampingRatio = 0.78f, stiffness = 1_000f)

/** FlClash 的橡皮筋映射：拉得越远增益越小，永远拉不出 [limit] */
internal fun rubberBand(overshoot: Float, limit: Float): Float {
    if (limit <= 0f) return 0f
    val pull = 1f - 1f / (abs(overshoot) * 0.55f / limit + 1f)
    return limit * pull * sign(overshoot)
}

/**
 * 阻尼弹簧把 [state] 从当前值驱向 [target]：半隐式欧拉逐帧积分（withFrameNanos 驱动）。
 * 拖拽跟手用 state 直接赋值，松手/点击后的弹性运移统一走这里；返回的 Job 可取消
 * （拖拽中赋值前先 cancel，避免新旧动画互相打架）。
 */
internal fun CoroutineScope.launchSpringTo(
    state: MutableFloatState,
    target: Float,
    stiffness: Float,
    dampingRatio: Float,
): Job = launch {
    var x = state.floatValue
    var velocity = 0f
    val c = 2f * sqrt(stiffness) * dampingRatio
    var lastNanos = 0L
    while (true) {
        val nanos = withFrameNanos { it }
        if (lastNanos == 0L) {
            lastNanos = nanos
            continue
        }
        val dt = ((nanos - lastNanos) / 1_000_000_000.0).toFloat().coerceIn(0f, 0.05f)
        lastNanos = nanos
        velocity += (stiffness * (target - x) - c * velocity) * dt
        x += velocity * dt
        if (abs(x - target) < 0.001f && abs(velocity) < 0.5f) {
            // 幅度小到看不见：贴目标结束，避免无限小振荡
            state.floatValue = target
            return@launch
        }
        state.floatValue = x
    }
}

/**
 * FlClash ElasticPress 风格的按压动效：
 * 按下弹性放大 → 手指拖动时按钮像橡皮筋一样跟手拉伸 → 松手带回弹收回去。
 *
 * 只作用于绘制（graphicsLayer），不改布局、不挪命中区域，手指在原位就能点到；
 * 手势被外层（如滚动列表）接管时拉伸照常进行（FlClash 的 Listener 不参与手势
 * 竞技场），只是抬手后不再算点击。无涟漪无水波，回弹本身就是按压反馈。
 *
 * 实现约束：`awaitEachGesture` 的块是受限挂起作用域（只能调 AwaitPointerEventScope
 * 自己的挂起函数），所以拖拽中用 state 直接赋值（非挂起）、回弹动画由协程跑
 * withFrameNanos 逐帧积分 —— 与官方 anchoredDraggable 同一架构。
 */
fun Modifier.elasticPress(
    enabled: Boolean = true,
    onClick: (() -> Unit)? = null,
): Modifier = composed {
    val lift = remember { Animatable(0f) }
    val pullX = remember { mutableFloatStateOf(0f) }
    val pullY = remember { mutableFloatStateOf(0f) }
    // PointerInputScope 不是 CoroutineScope：动画统一跑在与组合绑定的 scope 上
    val scope = rememberCoroutineScope()

    pointerInput(enabled) {
        var liftJob: Job? = null
        var settleJob: Job? = null

        fun liftTo(target: Float, spec: AnimationSpec<Float>) {
            liftJob?.cancel()
            liftJob = scope.launch { lift.animateTo(target, spec) }
        }

        /** 回弹：两轴各自阻尼弹簧积分回零 */
        suspend fun settleAxis(state: MutableFloatState) {
            var x = state.floatValue
            if (x == 0f) return
            var velocity = 0f
            val c = 2f * sqrt(SETTLE_STIFFNESS) * SETTLE_DAMPING
            var lastNanos = 0L
            while (true) {
                val nanos = withFrameNanos { it }
                if (lastNanos == 0L) {
                    lastNanos = nanos
                    continue
                }
                val dt = ((nanos - lastNanos) / 1_000_000_000.0).toFloat().coerceIn(0f, 0.05f)
                lastNanos = nanos
                velocity += (-SETTLE_STIFFNESS * x - c * velocity) * dt
                x += velocity * dt
                if (abs(x) < 0.01f && abs(velocity) < 10f) {
                    // 幅度小到看不见：直接贴零结束，避免无限小振荡
                    state.floatValue = 0f
                    return
                }
                state.floatValue = x
            }
        }

        fun settleAll() {
            liftTo(0f, LiftSpring)
            settleJob?.cancel()
            settleJob = scope.launch {
                launch { settleAxis(pullX) }
                launch { settleAxis(pullY) }
            }
        }

        awaitEachGesture {
            val down = awaitFirstDown(requireUnconsumed = false)
            if (!enabled) return@awaitEachGesture
            down.consume()
            settleJob?.cancel()
            liftTo(1f, LiftSpring)
            val origin = down.position
            // IntSize 没有 minDimension：取较短边（像素）算拖拽上限
            val limit = minOf(size.width, size.height).toFloat() * PULL_LIMIT_RATIO
            val slop = viewConfiguration.touchSlop
            var stolen = false // 手势被外层（滚动等）接管：拉伸照常，点击不算

            while (true) {
                val event = awaitPointerEvent()
                val change = event.changes.firstOrNull { it.id == down.id }
                if (change == null) {
                    // 追踪的指针不在事件里（异常中断）：弹回静止，不算点击
                    settleAll()
                    break
                }
                if (change.changedToUp()) {
                    change.consume()
                    settleAll()
                    val inside = !stolen &&
                        change.position.x >= -slop && change.position.x <= size.width + slop &&
                        change.position.y >= -slop && change.position.y <= size.height + slop
                    if (inside) onClick?.invoke()
                    break
                }
                // 外层（滚动等）接管手势也照样跟手拉伸 —— 滚动的同时按钮照样拉橡皮筋；
                // 只是抬手后不再算点击
                if (change.isConsumed) stolen = true
                val dx = change.position.x - origin.x
                val dy = change.position.y - origin.y
                // 跟手：橡皮筋映射后直接赋 state（非挂起，受限作用域内合法）
                pullX.floatValue = rubberBand(dx, limit)
                pullY.floatValue = rubberBand(dy, limit)
            }
        }
    }.graphicsLayer {
        // 每次值变化都会重算全部属性，避免归零后残留上一帧的缩放/位移
        if (size == Size.Zero) return@graphicsLayer
        val swell = 1f + lift.value *
            min(PRESS_GROWTH, MAX_PRESS_GROWTH_PX * density / size.maxDimension)
        scaleX = swell * (1f + abs(pullX.floatValue) / size.width * PULL_STRETCH)
        scaleY = swell * (1f + abs(pullY.floatValue) / size.height * PULL_STRETCH)
        translationX = pullX.floatValue
        translationY = pullY.floatValue
    }
}

/**
 * 带弹性按压动效的圆形图标按钮（FlClash 风格）。
 * [containerColor] 圆底色；[borderColor] 描边（透明 = 无描边）；禁用时整钮降透明度且不可点。
 */
@Composable
fun ElasticCircleButton(
    icon: ImageVector,
    contentDescription: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    diameter: Dp,
    iconSize: Dp,
    containerColor: Color,
    iconTintColor: Color,
    borderColor: Color = Color.Transparent,
    disabledAlpha: Float = 0.4f,
) {
    Box(
        modifier = modifier
            .alpha(if (enabled) 1f else disabledAlpha)
            .size(diameter)
            .elasticPress(enabled = enabled, onClick = onClick)
            .semantics { role = Role.Button }
            .clip(CircleShape)
            .background(containerColor)
            .then(
                if (borderColor == Color.Transparent) Modifier
                else Modifier.border(1.dp, borderColor, CircleShape),
            ),
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            icon,
            contentDescription = contentDescription,
            tint = iconTintColor,
            modifier = Modifier.size(iconSize),
        )
    }
}
