package io.juspay.sudoquery.sample

import android.app.Activity
import android.os.Bundle
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import io.juspay.sudoquery.SudoQuery

class MainActivity : Activity() {
    private var clicks = 0

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val status = TextView(this)
        val padding = (16 * resources.displayMetrics.density).toInt()
        val layout = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(padding, padding, padding, padding)
        }

        fun addButton(label: String, action: () -> String) {
            layout.addView(Button(this).apply {
                text = label
                setOnClickListener { status.text = action() }
            })
        }

        addButton("Track button_click") {
            clicks++
            SudoQuery.track("button_click", mapOf("button_id" to "sample", "clicks" to clicks))
            "Tracked button_click #$clicks"
        }
        addButton("Track 100 events") {
            repeat(100) { SudoQuery.track("bulk_event", mapOf("index" to it)) }
            "Tracked 100 events"
        }
        addButton("Set user user_123") {
            SudoQuery.setUser("user_123")
            "User: ${SudoQuery.getUser()}"
        }
        addButton("Remove user") {
            SudoQuery.removeUser()
            "User: ${SudoQuery.getUser()}"
        }
        addButton("Reset (logout)") {
            SudoQuery.reset()
            "Reset: new anonymous id and session"
        }
        addButton("Flush") {
            SudoQuery.flush()
            "Flush requested"
        }
        layout.addView(status)

        setContentView(ScrollView(this).apply {
            fitsSystemWindows = true
            addView(layout)
        })
    }
}
