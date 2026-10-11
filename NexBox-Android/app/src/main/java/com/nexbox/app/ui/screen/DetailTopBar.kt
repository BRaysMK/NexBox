package com.nexbox.app.ui.screen

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.util.lerp
import com.kyant.backdrop.drawBackdrop
import com.kyant.backdrop.effects.blur
import com.kyant.backdrop.effects.lens
import com.kyant.backdrop.effects.vibrancy
import com.nexbox.app.data.AppearanceStore
import com.nexbox.app.ui.LocalCardBackdrop
import com.nexbox.app.ui.elasticPress
import com.nexbox.app.ui.glass.InteractiveHighlight
import com.nexbox.app.ui.theme.GlassBorder
import com.nexbox.app.ui.theme.Hairline
import com.nexbox.app.ui.theme.TextSecondary
import kotlin.math.abs
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.sin
import kotlin.math.tanh

/**
 * 详情页悬浮头骨架（FlClash FloatingHeader 式）：
 * - 内容层铺满全屏，滚动内容以全透明度从悬浮头与系统状态栏后面穿过（完全贴边），
 *   状态栏图标浮在内容上
 * - 悬浮头：液态玻璃/实色返回圆钮 + 标题 + 右侧时效标注，弹性按压
 */
@Composable
internal fun DetailScaffold(
    title: String,
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
    trailingText: String? = null,
    /** 盖在最上层的浮层（toast 等） */
    overlay: @Composable BoxScope.() -> Unit = {},
    content: @Composable BoxScope.() -> Unit,
) {
    Box(modifier.fillMaxSize()) {
        content()
        Row(
            modifier = Modifier
                .align(Alignment.TopCenter)
                .fillMaxWidth()
                .statusBarsPadding()
                .padding(horizontal = 14.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            FloatingBackButton(onBack)
            Spacer(Modifier.width(12.dp))
            Text(
                title,
                fontSize = 19.sp,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onBackground,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f),
            )
            if (trailingText != null) {
                Spacer(Modifier.width(8.dp))
                Text(trailingText, fontSize = 11.sp, color = TextSecondary)
            }
        }
        overlay()
    }
}

/** 内容首个元素与悬浮头的净距：状态栏 + 头部行高 + 呼吸空隙 */
@Composable
internal fun Modifier.detailHeaderSpacing(): Modifier {
    val barTop = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
    return padding(top = barTop + 72.dp)
}

/** 主页面（无返回钮）内容首元素与悬浮标题的净距：状态栏 + 标题行高 + 呼吸空隙 */
@Composable
internal fun Modifier.pageHeaderSpacing(): Modifier {
    val barTop = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
    return padding(top = barTop + 64.dp)
}
/** 滚动内容顶部渐隐蒙版：内容滚进状态栏 + 悬浮头区域时逐渐透明（背景层不参与，不跟着渐隐）。
 *  必须挂在滚动容器**外侧**（verticalScroll 之前），蒙版才固定在屏幕顶部、不随内容滚动 */
@Composable
internal fun Modifier.contentTopFade(): Modifier {
    val barTop = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
    val fadePx = with(LocalDensity.current) { (barTop + 64.dp).toPx() }
    return this
        .graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
        .drawWithContent {
            drawContent()
            drawRect(
                brush = Brush.verticalGradient(
                    colors = listOf(Color.Transparent, Color.Black),
                    startY = 0f,
                    endY = fadePx,
                ),
                blendMode = BlendMode.DstIn,
            )
        }
}

/** 主页面悬浮标题：状态栏下方悬浮、不随内容滚动（FlClash FloatingHeader 同款） */
@Composable
internal fun FloatingPageTitle(title: String, modifier: Modifier = Modifier) {
    Text(
        title,
        fontSize = 20.sp,
        fontWeight = FontWeight.SemiBold,
        color = MaterialTheme.colorScheme.onBackground,
        modifier = modifier
            .fillMaxWidth()
            .statusBarsPadding()
            .padding(horizontal = 18.dp, vertical = 14.dp),
    )
}

/**
 * 悬浮返回钮，与 AndroidLiquidGlass-kmp 的 LiquidButton 同款：
 * - 玻璃开：drawBackdrop（vibrancy + 2dp 轻模糊 + 边缘折射）+ InteractiveHighlight
 *   液态形变（按下朝指尖位移、沿拖拽方向挤扁拉伸）——小钮用 8dp 模糊会彻底糊死透不出背景
 * - 玻璃关：主题 surface 纯色圆底 + 弹性按压
 */
@Composable
internal fun FloatingBackButton(onBack: () -> Unit) {
    val backdrop = LocalCardBackdrop.current
    val glass = AppearanceStore.glassEnabled && backdrop != null
    val animationScope = rememberCoroutineScope()
    val interactiveHighlight = remember(animationScope) { InteractiveHighlight(animationScope) }
    val core = Modifier
        .size(40.dp)
        .then(
            if (glass) {
                Modifier
                    .drawBackdrop(
                        backdrop = backdrop,
                        shape = { CircleShape },
                        effects = {
                            vibrancy()
                            blur(2f.dp.toPx())
                            lens(12f.dp.toPx(), 24f.dp.toPx())
                        },
                        layerBlock = {
                            val width = size.width
                            val height = size.height

                            val progress = interactiveHighlight.pressProgress
                            val scale = lerp(1f, 1f + 4f.dp.toPx() / size.height, progress)

                            val maxOffset = size.minDimension
                            val initialDerivative = 0.05f
                            val offset = interactiveHighlight.offset
                            translationX = maxOffset * tanh(initialDerivative * offset.x / maxOffset).toFloat()
                            translationY = maxOffset * tanh(initialDerivative * offset.y / maxOffset).toFloat()

                            val maxDragScale = 4f.dp.toPx() / size.height
                            val offsetAngle = atan2(offset.y, offset.x)
                            scaleX =
                                scale +
                                    maxDragScale * abs(cos(offsetAngle) * offset.x / size.maxDimension) *
                                    (width / height).coerceAtMost(1f)
                            scaleY =
                                scale +
                                    maxDragScale * abs(sin(offsetAngle) * offset.y / size.maxDimension) *
                                    (height / width).coerceAtMost(1f)
                        },
                    )
                    .clickable(interactionSource = null, indication = null, onClick = onBack)
                    .then(interactiveHighlight.modifier)
                    .then(interactiveHighlight.gestureModifier)
                    .border(1.dp, GlassBorder, CircleShape)
            } else {
                Modifier
                    .elasticPress(onClick = onBack)
                    .clip(CircleShape)
                    .background(MaterialTheme.colorScheme.surface)
                    .border(1.dp, Hairline, CircleShape)
            },
        )
    Box(core, contentAlignment = Alignment.Center) {
        Icon(
            Icons.AutoMirrored.Rounded.ArrowBack,
            contentDescription = "返回",
            tint = MaterialTheme.colorScheme.onBackground,
            modifier = Modifier.size(22.dp),
        )
    }
}

/** 未迁移页面的兼容占位已移除：详情页骨架统一走 [DetailScaffold] */
