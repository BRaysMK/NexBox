package com.nexbox.app.ui

import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.composed

/**
 * 弹窗「点空白关闭」组合（与全屏 Box + 中央卡片的自绘弹窗配套）：
 *
 * Compose Dialog 的 onDismissRequest 只在点击 **dialog 窗口之外** 时触发，
 * 而这类弹窗的内容铺满全屏（fillMaxSize Box 居中一张卡片），窗口永远没有
 * "之外"，系统给的点外部关闭就永久失效了。
 *
 * 用法（两个 modifier 成对）：
 * - 外层 Box（scrim 层）挂 [dialogScrimClick]：点空白触发 onDismiss；
 * - 中央卡片挂 [consumeDialogClick]：把落在卡片上的点击消费掉，
 *   避免冒泡到 scrim 误关（卡片内部按钮/滑杆各自优先消费，不受影响）。
 */
fun Modifier.dialogScrimClick(onDismiss: () -> Unit): Modifier = composed {
    clickable(
        interactionSource = remember { MutableInteractionSource() },
        indication = null,
    ) { onDismiss() }
}

/** 弹窗卡片消费点击用：无反馈、无动作，仅阻断向 scrim 层冒泡（见 [dialogScrimClick]） */
fun Modifier.consumeDialogClick(): Modifier = composed {
    clickable(
        interactionSource = remember { MutableInteractionSource() },
        indication = null,
    ) {}
}
