package com.nexbox.app

import android.os.Build
import android.os.Bundle
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.core.view.WindowCompat
import com.nexbox.app.data.ClipboardSyncManager
import com.nexbox.app.ui.NexBoxApp

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // edge-to-edge：让系统栏与内容共存，配合 WindowInsets 处理
        WindowCompat.setDecorFitsSystemWindows(window, false)
        // 刘海/挖孔区域也要绘制：默认 portrait 下窗口避让 cutout，屏幕顶部会留一条黑带
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            window.attributes.layoutInDisplayCutoutMode =
                WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
        }
        setContent {
            NexBoxApp()
        }
    }

    override fun onStart() {
        super.onStart()
        // 剪贴板同步：进前台（Android 10+ 只有有焦点的窗口才能读写剪贴板）
        ClipboardSyncManager.onAppForeground()
    }

    override fun onStop() {
        super.onStop()
        ClipboardSyncManager.onAppBackground()
    }
}
