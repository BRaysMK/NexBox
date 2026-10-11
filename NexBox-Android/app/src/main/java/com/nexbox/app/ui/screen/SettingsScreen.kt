package com.nexbox.app.ui.screen

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Image as LogoImage
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.windowInsetsTopHeight
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Refresh
import androidx.compose.material.icons.rounded.BlurOn
import androidx.compose.material.icons.rounded.Code
import androidx.compose.material.icons.rounded.Language
import androidx.compose.material.icons.rounded.Colorize
import androidx.compose.material.icons.rounded.Done
import androidx.compose.material.icons.rounded.Image
import androidx.compose.material.icons.rounded.OpenInNew
import androidx.compose.material.icons.rounded.Palette
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import coil3.compose.AsyncImage
import com.nexbox.app.BuildConfig
import com.nexbox.app.R
import com.nexbox.app.data.AppearanceStore
import com.nexbox.app.data.BackgroundStore
import com.nexbox.app.ui.elasticPress
import com.nexbox.app.data.DefaultAccentHex
import com.nexbox.app.data.ThemeMode
import com.nexbox.app.ui.AppCard
import com.nexbox.app.ui.theme.Accent
import com.nexbox.app.ui.theme.AccentPresets
import com.nexbox.app.ui.theme.Danger
import com.nexbox.app.ui.theme.Hairline
import com.nexbox.app.ui.theme.PageBg
import com.nexbox.app.ui.theme.PageBgTop
import com.nexbox.app.ui.theme.TextSecondary
import com.nexbox.app.ui.theme.accentContentColor
import com.nexbox.app.ui.theme.isDarkTheme
import com.nexbox.app.ui.theme.parseHexColor
import com.nexbox.app.update.UpdateManager

/** 设置页：主题模式 / 主题色 / 外观 / 液态玻璃 / 关于 */
@Composable
fun SettingsScreen() {
    val context = LocalContext.current
    var pickingAccent by remember { mutableStateOf(false) }

    SettingsList(onOpenPicker = { pickingAccent = true })

    if (pickingAccent) {
        AccentPickerDialog(
            initialHex = AppearanceStore.accent,
            defaultHex = DefaultAccentHex,
            onApply = { hex ->
                AppearanceStore.setAccent(context, hex)
                pickingAccent = false
            },
            onDismiss = { pickingAccent = false },
        )
    }
}

@Composable
private fun SettingsList(onOpenPicker: () -> Unit) {
    val context = LocalContext.current
    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 18.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        // 状态栏高度用滚动内边距顶开，而不是滚动容器避让状态栏：
        // 内容滚动时能完整穿到状态栏底下，顶部不再被割裂
        Spacer(Modifier.windowInsetsTopHeight(WindowInsets.statusBars))
        Spacer(Modifier.height(2.dp))
        Text(
            "设置",
            fontSize = 20.sp,
            fontWeight = FontWeight.SemiBold,
            color = MaterialTheme.colorScheme.onBackground,
        )
AppCard(modifier = Modifier.fillMaxWidth(), shape = RoundedCornerShape(16.dp)) {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                SectionHead(Icons.Rounded.Palette, "主题模式")
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    ThemeMode.entries.forEach { mode ->
                        ModeChip(
                            label = mode.label,
                            selected = AppearanceStore.themeMode == mode,
                            modifier = Modifier.weight(1f),
                        ) { AppearanceStore.setThemeMode(context, mode) }
                    }
                }
                Text(
                    "跟随系统时按系统的深色设置切换",
                    fontSize = 11.5.sp,
                    color = TextSecondary,
                )
            }
        }

        AppCard(modifier = Modifier.fillMaxWidth(), shape = RoundedCornerShape(16.dp)) {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                SectionHead(Icons.Rounded.Colorize, "主题色")
                // 色块尺寸按可用宽度算，但**封顶 44dp**：
                // 平板 / 横屏下卡片很宽，纯 weight(1f) 会把这一排色块撑成一堆大圆饼。
                BoxWithConstraints {
                    val gap = 10.dp
                    // 末尾还有一个方形调色盘入口，一起参与均分
                    val count = AccentPresets.size + 1
                    val swatch = minOf((maxWidth - gap * (count - 1)) / count, 44.dp)
                    Row(horizontalArrangement = Arrangement.spacedBy(gap)) {
                        AccentPresets.forEach { hex ->
                            val color = parseHexColor(hex)
                            if (color != null) {
                                AccentSwatch(
                                    color = color,
                                    selected = hex.equals(AppearanceStore.accent, ignoreCase = true),
                                    modifier = Modifier.size(swatch),
                                ) { AppearanceStore.setAccent(context, hex) }
                            }
                        }
                        // 方形是为了和圆形的预设区分开：它是入口，不是颜色
                        PaletteSquare(modifier = Modifier.size(swatch), onClick = onOpenPicker)
                    }
                }
            }
        }

        AppCard(modifier = Modifier.fillMaxWidth(), shape = RoundedCornerShape(16.dp)) {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                SectionHead(Icons.Rounded.Image, "外观")
                BackgroundPicker()
            }
        }

        AppCard(modifier = Modifier.fillMaxWidth(), shape = RoundedCornerShape(16.dp)) {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                SectionHead(Icons.Rounded.BlurOn, "液态玻璃")
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                        Text(
                            "液态玻璃（需安卓13及以上）",
                            fontSize = 13.sp,
                            fontWeight = FontWeight.Medium,
                            color = MaterialTheme.colorScheme.onSurface,
                        )
                        Text(
                            "开启后界面卡片将启用液态玻璃效果，拥有模糊和真实的折射，但会大幅增加性能负载和功耗。",
                            fontSize = 11.5.sp,
                            color = TextSecondary,
                        )
                    }
                    Switch(
                        checked = AppearanceStore.glassEnabled,
                        onCheckedChange = { AppearanceStore.setGlassEnabled(context, it) },
                    )
                }
            }
        }

        // 关于：居中头部（双 Logo + 版本徽标 + 检查更新）+ 更多链接
        AboutHeader()
        AboutLinks()

        Spacer(Modifier.height(8.dp))
        // 悬浮玻璃导航条的余量：内容从它下面穿过，末尾留出高度不被永久遮住
        Spacer(Modifier.navigationBarsPadding().height(96.dp))
    }
}

/** 卡片标题：小图标 + 13sp 标题，和硬件页的分节保持同一层级 */
@Composable
private fun SectionHead(icon: ImageVector?, title: String) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        if (icon != null) {
            Icon(icon, contentDescription = null, tint = Accent, modifier = Modifier.size(16.dp))
            Spacer(Modifier.width(7.dp))
        }
        Text(
            title,
            fontSize = 13.sp,
            fontWeight = FontWeight.SemiBold,
            color = MaterialTheme.colorScheme.onSurface,
        )
    }
}

@Composable
private fun ModeChip(label: String, selected: Boolean, modifier: Modifier = Modifier, onClick: () -> Unit) {
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(12.dp))
            .background(
                if (selected) MaterialTheme.colorScheme.primary.copy(alpha = 0.16f)
                else MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.45f),
            )
            .border(
                width = if (selected) 1.dp else 0.dp,
                color = if (selected) MaterialTheme.colorScheme.primary else Hairline,
                shape = RoundedCornerShape(12.dp),
            )
            .clickable(onClick = onClick)
            .padding(vertical = 11.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            label,
            fontSize = 12.5.sp,
            fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal,
            color = if (selected) MaterialTheme.colorScheme.primary else TextSecondary,
        )
    }
}

/** 背景图选择：相册选图后拷进应用私有目录，重启仍然生效 */
@Composable
private fun BackgroundPicker() {
    val context = LocalContext.current
    val bgFile = BackgroundStore.file
    // 选图读不到数据（云端图片、相册权限失效等）时要有可见反馈，
    // 否则点了「更换背景图」界面一动不动，用户只会认为没生效
    var bgError by remember { mutableStateOf<String?>(null) }
    val pickBackground = rememberLauncherForActivityResult(
        ActivityResultContracts.PickVisualMedia(),
    ) { uri ->
        if (uri == null) {
            bgError = null
        } else {
            bgError = if (BackgroundStore.setFromUri(context, uri)) {
                null
            } else {
                "读取所选图片失败，背景未更换。图片若存放在云端，请先下载到本机再试"
            }
        }
    }

    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Box(
            Modifier
                .size(width = 84.dp, height = 52.dp)
                .clip(RoundedCornerShape(10.dp))
                .background(Brush.verticalGradient(listOf(PageBgTop, PageBg))),
        ) {
            if (bgFile != null) {
                AsyncImage(
                    model = bgFile,
                    contentDescription = null,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.fillMaxSize(),
                )
            }
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
            Text(
                if (bgFile != null) "自定义背景" else "默认背景",
                fontSize = 13.sp,
                fontWeight = FontWeight.Medium,
                color = MaterialTheme.colorScheme.onSurface,
            )
            Text(
                if (bgFile != null) "整应用生效" else "跟随主题模式的渐变色",
                fontSize = 11.5.sp,
                color = TextSecondary,
            )
        }
    }

    OutlinedButton(
        onClick = {
            pickBackground.launch(
                PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly),
            )
        },
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(12.dp),
    ) { Text(if (bgFile != null) "更换背景图" else "选择背景图") }

    if (bgFile != null) {
        OutlinedButton(
            onClick = {
                BackgroundStore.clear()
                bgError = null
            },
            modifier = Modifier.fillMaxWidth(),
            shape = RoundedCornerShape(12.dp),
        ) { Text("恢复默认背景") }
    }

    bgError?.let { Text(it, fontSize = 11.5.sp, color = Danger) }
}

/** 关于头部：双 Logo + 版本徽标 + 检查更新胶囊按钮（FlClash 式居中排版） */
@Composable
private fun AboutHeader() {
    AppCard(modifier = Modifier.fillMaxWidth(), shape = RoundedCornerShape(16.dp)) {
        Column(
            modifier = Modifier.fillMaxWidth().padding(vertical = 20.dp, horizontal = 16.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            // 黑色双 Logo（图形标识 + 中文标准字）
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                LogoImage(
                    painter = painterResource(R.drawable.logo_nexbox_black),
                    colorFilter = if (isDarkTheme()) ColorFilter.tint(Color.White) else null,
                    contentDescription = "新境盒标识",
                    contentScale = ContentScale.Fit,
                    modifier = Modifier.size(48.dp),
                )
                LogoImage(
                    painter = painterResource(R.drawable.logo_chinese_black),
                    colorFilter = if (isDarkTheme()) ColorFilter.tint(Color.White) else null,
                    contentDescription = "新境盒",
                    contentScale = ContentScale.Fit,
                    modifier = Modifier.size(width = 133.dp, height = 44.5.dp),
                )
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                VersionChip("v${BuildConfig.VERSION_NAME}")
                VersionChip("GPL-3")
            }
            UpdateCheckButton()
        }
    }
}

@Composable
private fun VersionChip(text: String) {
    Box(
        modifier = Modifier
            .clip(RoundedCornerShape(50))
            .background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.6f))
            .padding(horizontal = 10.dp, vertical = 4.dp),
    ) {
        Text(
            text,
            fontSize = 11.5.sp,
            fontWeight = FontWeight.SemiBold,
            color = MaterialTheme.colorScheme.onSurface,
        )
    }
}

/**
 * 检查更新胶囊按钮：文案跟随更新状态机变化。
 * 发现新版本时变成「发现新版本」，点击把更新弹窗拉起来；空闲 / 已最新时发起手动检查。
 */
@Composable
private fun UpdateCheckButton() {
    val enabled: Boolean
    val label: String
    when (val s = UpdateManager.state) {
        is UpdateManager.State.Checking -> { enabled = false; label = "检查中…" }
        is UpdateManager.State.Available -> { enabled = true; label = "发现新版本" }
        is UpdateManager.State.Downloading -> { enabled = false; label = "下载中 ${s.progress}%" }
        is UpdateManager.State.Downloaded -> { enabled = true; label = "待安装" }
        is UpdateManager.State.Failed -> { enabled = true; label = "重试" }
        UpdateManager.State.Idle, UpdateManager.State.UpToDate -> { enabled = true; label = "检查更新" }
    }
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(7.dp),
        modifier = Modifier
            .clip(RoundedCornerShape(50))
            .background(MaterialTheme.colorScheme.primary)
            .clickable(enabled = enabled) { UpdateManager.manualCheck() }
            .padding(horizontal = 13.dp, vertical = 6.dp),
    ) {
        Icon(
            Icons.Rounded.Refresh,
            contentDescription = null,
            tint = accentContentColor(MaterialTheme.colorScheme.primary),
            modifier = Modifier.size(13.dp),
        )
        Text(
            label,
            fontSize = 11.5.sp,
            fontWeight = FontWeight.SemiBold,
            color = accentContentColor(MaterialTheme.colorScheme.primary),
        )
    }
}

/** 「更多」链接卡：官网 / 本项目 / 液态玻璃效果来源 */
@Composable
private fun AboutLinks() {
    Column {
        AppCard(modifier = Modifier.fillMaxWidth(), shape = RoundedCornerShape(16.dp)) {
            Column {
                LinkRow(Icons.Rounded.Language, "官网", "nexbox.cn", "https://nexbox.cn")
                HorizontalDivider(color = Hairline)
                LinkRow(Icons.Rounded.Code, "本项目", "github.com/MuLiuSaMa/NexBox", "https://github.com/MuLiuSaMa/NexBox")
                HorizontalDivider(color = Hairline)
                LinkRow(
                    Icons.Rounded.BlurOn,
                    "液态玻璃",
                    "Kyant0/AndroidLiquidGlass",
                    "https://github.com/Kyant0/AndroidLiquidGlass/tree/android",
                )
            }
        }
    }
}

@Composable
private fun LinkRow(icon: ImageVector, title: String, subtitle: String, url: String) {
    val uriHandler = LocalUriHandler.current
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable { uriHandler.openUri(url) }
            .padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Box(
            modifier = Modifier
                .size(34.dp)
                .clip(RoundedCornerShape(percent = 32))
                .background(Accent.copy(alpha = 0.16f)),
            contentAlignment = Alignment.Center,
        ) {
            Icon(icon, contentDescription = null, tint = Accent, modifier = Modifier.size(18.dp))
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Text(
                title,
                fontSize = 13.sp,
                fontWeight = FontWeight.Medium,
                color = MaterialTheme.colorScheme.onSurface,
            )
            Text(subtitle, fontSize = 11.5.sp, color = TextSecondary)
        }
        Icon(
            Icons.Rounded.OpenInNew,
            contentDescription = "打开链接",
            tint = TextSecondary,
            modifier = Modifier.size(16.dp),
        )
    }
}
/** 预设色块：选中时套一圈跟随主题的描边，深浅色背景上都看得见 */
@Composable
private fun AccentSwatch(
    color: Color,
    selected: Boolean,
    modifier: Modifier = Modifier,
    onClick: () -> Unit,
) {
    Box(
        modifier = modifier
            .elasticPress(onClick = onClick)
            .border(
                width = if (selected) 2.dp else 0.dp,
                color = MaterialTheme.colorScheme.onSurface.copy(alpha = 0.85f),
                shape = CircleShape,
            )
            .padding(if (selected) 3.dp else 0.dp)
            .clip(CircleShape)
            .background(color),
        contentAlignment = Alignment.Center,
    ) {
        if (selected) {
            Icon(
                Icons.Rounded.Done,
                contentDescription = "已选",
                tint = accentContentColor(color),
                modifier = Modifier.size(15.dp),
            )
        }
    }
}

/** 调色盘入口：方形 + 彩虹渐变，和圆形的预设色块一眼分开 */
@Composable
private fun PaletteSquare(modifier: Modifier = Modifier, onClick: () -> Unit) {
    val spectrum = AccentPresets.mapNotNull { parseHexColor(it) }
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(9.dp))
            .background(Brush.linearGradient(spectrum))
            .border(1.dp, MaterialTheme.colorScheme.outlineVariant, RoundedCornerShape(9.dp))
            .clickable(onClickLabel = "打开调色盘", onClick = onClick),
    )
}
