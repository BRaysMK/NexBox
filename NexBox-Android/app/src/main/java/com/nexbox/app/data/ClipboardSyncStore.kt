package com.nexbox.app.data

import android.content.Context
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

/**
 * 剪贴板同步开关（SharedPreferences 持久化，与 OverlayStore 同款模式）。
 *
 * 开关语义（用户确认）：**只控制发送方向**（本机剪贴板 → PC）；
 * 接收方向（PC → 本机）始终开启，不受此开关影响。
 * Compose 状态直接驱动主页卡片 Switch，改动即时生效。
 */
object ClipboardSyncStore {
    private const val PREFS_NAME = "nexbox_clipboard_sync"
    private const val KEY_ENABLED = "enabled"
    private const val KEY_HINT_SHOWN = "hint_shown"

    var enabled by mutableStateOf(false)
        private set

    /** 「仅前台同步」首次开启提示是否已弹过（只弹一次） */
    var hintShown by mutableStateOf(false)
        private set

    fun init(context: Context) {
        val p = prefs(context)
        enabled = p.getBoolean(KEY_ENABLED, false)
        hintShown = p.getBoolean(KEY_HINT_SHOWN, false)
    }

    fun setEnabled(context: Context, on: Boolean) {
        enabled = on
        prefs(context).edit().putBoolean(KEY_ENABLED, on).apply()
        // 通知管理器启停发送轮询
        ClipboardSyncManager.onEnabledChanged()
    }

    fun markHintShown(context: Context) {
        hintShown = true
        prefs(context).edit().putBoolean(KEY_HINT_SHOWN, true).apply()
    }

    private fun prefs(context: Context) =
        context.applicationContext.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
}
