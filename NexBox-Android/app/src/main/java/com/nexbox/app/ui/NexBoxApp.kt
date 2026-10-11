package com.nexbox.app.ui

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.Image
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.nexbox.app.data.AppearanceStore
import com.nexbox.app.ui.screen.ConfigScreen
import com.nexbox.app.ui.screen.DeltaForceScreen
import com.nexbox.app.ui.screen.FunctionScreen
import com.nexbox.app.ui.screen.HomeScreen
import com.nexbox.app.ui.screen.SettingsScreen
import com.nexbox.app.ui.theme.NexBoxTheme
import com.nexbox.app.ui.theme.TextPrimary
import com.kyant.backdrop.backdrops.layerBackdrop
import com.kyant.backdrop.backdrops.rememberLayerBackdrop
import com.nexbox.app.update.UpdateManager
import com.nexbox.app.ui.glass.LiquidBottomTabs
import com.nexbox.app.ui.theme.isDarkTheme
import kotlinx.coroutines.delay

@Composable
fun NexBoxApp() {
    NexBoxTheme {
        var tab by remember { mutableStateOf(NexBoxTab.Home) }
        var booting by remember { mutableStateOf(true) }
        val glass = AppearanceStore.glassEnabled


        // 全局状态栏渐隐：所有页面内容滚进状态栏区域时逐渐透明；
        // 背景层（AppBackground）独立在外，不参与蒙版，所以背景不渐隐
        val density = LocalDensity.current
        val barTop = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
        val statusBarFadePx = with(density) { barTop.toPx() }
        // 液态玻璃取样源（layerBackdrop 记录所在层的绘制内容）：
        // - cardBackdrop 只挂背景层：玻璃卡片只折射/模糊壁纸与渐变，不采样自己
        // - contentBackdrop 挂背景+内容：玻璃导航条采样它，内容从条下穿过时照样被折射
        // 玻璃关闭时不挂载，零开销
        val cardBackdrop = rememberLayerBackdrop()
        val contentBackdrop = rememberLayerBackdrop()
        LaunchedEffect(Unit) {
            delay(2200)
            booting = false
        }

        // 开屏结束后再静默检查更新：不抢启动的网络与渲染，失败 / 已是最新都保持安静
        LaunchedEffect(booting) {
            if (!booting) {
                delay(1500)
                UpdateManager.autoCheck()
            }
        }

        // 「安装未知应用」授权页返回前台后自动续装（UpdateManager 内部判断 pending 标记）
        val lifecycleOwner = LocalLifecycleOwner.current
        DisposableEffect(lifecycleOwner) {
            val observer = LifecycleEventObserver { _, event ->
                if (event == Lifecycle.Event.ON_RESUME) UpdateManager.onHostResumed()
            }
            lifecycleOwner.lifecycle.addObserver(observer)
            onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
        }

        CompositionLocalProvider(LocalCardBackdrop provides cardBackdrop) {
            Box(Modifier.fillMaxSize()) {
                // 背景 + 页面内容整体作为玻璃导航条的光学取样源：
                // 折射/模糊要能吃到背景图，所以取样源包住背景层
                Box(
                    Modifier
                        .fillMaxSize()
                        .then(if (glass) Modifier.layerBackdrop(contentBackdrop) else Modifier),
                ) {
                    // 背景层：设置页选的背景图优先，否则默认深色渐变（AppBackground 统一渲染）。
                    // 单独再挂一个取样源（cardBackdrop）：玻璃卡片只采样背景层，
                    // 不采样页面内容，避免卡片把自己的内容糊进自己
                    Box(
                        Modifier
                            .fillMaxSize()
                            .then(if (glass) Modifier.layerBackdrop(cardBackdrop) else Modifier),
                    ) {
                        AppBackground(Modifier.fillMaxSize())
                    }

                    // 页面铺满整个屏幕，底部不再为导航条预留实色区域——
                    // 那样会把内容「割断」。内容从悬浮玻璃条下面穿过去。
                    AnimatedContent(
                        targetState = tab,
                        transitionSpec = {
                            // 时长压短：转场期间两个页面都参与合成，拖得越久越容易撞上
                            // 硬件数据每秒刷新带来的重组、掉帧越明显
                            val dur = 200
                            (fadeIn(animationSpec = tween(dur)) +
                                slideInHorizontally(animationSpec = tween(dur)) { it / 10 })
                                .togetherWith(fadeOut(animationSpec = tween(140)))
                        },
                        label = "nexbox-root",
                        modifier = Modifier
                            .fillMaxSize()
                            .graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
                            .drawWithContent {
                                drawContent()
                                drawRect(
                                    brush = Brush.verticalGradient(
                                        colors = listOf(Color.Transparent, Color.Black),
                                        startY = 0f,
                                        endY = statusBarFadePx,
                                    ),
                                    blendMode = BlendMode.DstIn,
                                )
                            },
                    ) { current ->
                        when (current) {
                            NexBoxTab.Home -> HomeScreen()
                            NexBoxTab.Config -> ConfigScreen()
                            NexBoxTab.Function -> FunctionScreen()
                            NexBoxTab.Delta -> DeltaForceScreen()
                            NexBoxTab.Settings -> SettingsScreen()
                        }
                    }
                }

                // 底部导航：玻璃开 = 胶囊式液态玻璃导航（LiquidBottomTabs），关 = 实色悬浮条
                if (glass) {
                    val dark = isDarkTheme()
                    LiquidBottomTabs(
                        selectedTabIndex = { NexBoxTab.entries.indexOf(tab) },
                        onTabSelected = { index -> tab = NexBoxTab.entries[index] },
                        backdrop = contentBackdrop,
                        tabsCount = NexBoxTab.entries.size,
                        containerColor = if (dark) Color(0xFF121212).copy(alpha = 0.4f)
                        else Color(0xFFFAFAFA).copy(alpha = 0.4f),
                        accentColor = MaterialTheme.colorScheme.primary,
                        pillColor = if (dark) Color.White.copy(alpha = 0.1f)
                        else Color.Black.copy(alpha = 0.1f),
                        modifier = Modifier
                            .align(Alignment.BottomCenter)
                            .fillMaxWidth()
                            .navigationBarsPadding()
                            .padding(horizontal = 14.dp, vertical = 10.dp),
                    ) { index ->
                        CapsuleTabContent(item = NexBoxTab.entries[index], selected = tab == NexBoxTab.entries[index])
                    }
                } else {
                    SolidDock(
                        selectedIndex = NexBoxTab.entries.indexOf(tab),
                        onSelected = { index -> tab = NexBoxTab.entries[index] },
                        tabsCount = NexBoxTab.entries.size,
                        barColor = MaterialTheme.colorScheme.surface,
                        pillColor = MaterialTheme.colorScheme.primary.copy(alpha = 0.15f),
                        contentColor = TextPrimary.copy(alpha = 0.6f),
                        accentColor = MaterialTheme.colorScheme.primary,
                        modifier = Modifier
                            .align(Alignment.BottomCenter)
                            .fillMaxWidth()
                            .navigationBarsPadding()
                            .padding(horizontal = 14.dp, vertical = 10.dp),
                    ) { index, color ->
                        SolidTabContent(NexBoxTab.entries[index], color)
                    }
                }

                // 开屏动画覆盖层（到时淡出）
                AnimatedVisibility(
                    visible = booting,
                    enter = fadeIn(),
                    exit = fadeOut(animationSpec = tween(400)),
                ) {
                    SplashScreen()
                }

                // 更新弹窗 + 更新轻提示：挂在根部，任何 tab 下都盖得住
                UpdateDialog()
                MessageToast(
                    message = UpdateManager.toast,
                    isError = false,
                    onDismiss = { UpdateManager.clearToast() },
                    modifier = Modifier
                        .align(Alignment.TopCenter)
                        .statusBarsPadding()
                        .padding(top = 10.dp, start = 18.dp, end = 18.dp),
                )
            }
        }
    }
}

/**
 * 胶囊导航里的单个标签内容：图标 + 文字，选中时用主题色。
 * 与实色导航的 NavBarItem 不同：没有自己的背景块——选中态由液态滑块表达。
 */
@Composable
private fun CapsuleTabContent(item: NexBoxTab, selected: Boolean) {
    val fg by animateColorAsState(
        targetValue = if (selected) MaterialTheme.colorScheme.primary else TextPrimary.copy(alpha = 0.6f),
        label = "nav-capsule-item-fg",
    )
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        // 图标下移一点，和文字更紧凑；offset 不参与布局，文字位置不动
        TabGlyph(item = item, tint = fg, modifier = Modifier.offset(y = 3.dp))
        Text(
            item.label,
            fontSize = 10.5.sp,
            fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal,
            color = fg,
        )
    }
}


/**
 * 实色导航坞的单个标签内容：与 [CapsuleTabContent] 同构，但颜色由 SolidDock 显式传入
 * （正常色 / 滑块扫过的主题色染色副本），不做选中态动画——选中感由滑块表达。
 */
@Composable
private fun SolidTabContent(item: NexBoxTab, color: Color) {
    Column(
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        TabGlyph(item = item, tint = color, modifier = Modifier.offset(y = 3.dp))
        Text(item.label, fontSize = 10.5.sp, color = color)
    }
}


/**
 * 导航图标：矢量走 [Icon]，位图 logo（三角洲）走 [painterResource]。
 * 两条分支都吃同一个 tint，所以选中/未选中的颜色动画对两者完全一致；
 * 位图那份是白色剪影，tint 之后和 Material 图标的光学重量才对得上。
 */
@Composable
internal fun TabGlyph(item: NexBoxTab, tint: Color, modifier: Modifier = Modifier) {
    if (item.bitmapIcon == 0) {
        Icon(item.icon, contentDescription = item.label, tint = tint, modifier = modifier)
    } else {
        Image(
            painter = painterResource(item.bitmapIcon),
            contentDescription = item.label,
            colorFilter = ColorFilter.tint(tint),
            // 素材是 282×207 的横图，Inside 等比缩放后实际高度只有方框的 0.73 倍；
            // 给到 28dp 才和旁边 24dp 的 Material 图标笔画重量相当
            contentScale = ContentScale.Inside,
            modifier = modifier.size(28.dp),
        )
    }
}
