//! Clipboard operations with protection and restoration
//! Saves original clipboard content before operations and restores it afterward

use anyhow::Result;
use arboard::Clipboard;
use std::thread;
use std::time::Duration;

/// Guard that saves clipboard content on creation and restores it on drop
pub struct ClipboardGuard {
    original_text: Option<String>,
}

impl ClipboardGuard {
    /// Create a new guard, saving the current clipboard content
    pub fn new() -> Self {
        let original_text = Clipboard::new()
            .ok()
            .and_then(|mut cb| cb.get_text().ok());

        Self { original_text }
    }

    /// Get text from clipboard (after Ctrl+C has been sent)
    pub fn get_text(&self) -> Result<String> {
        // Small delay to ensure clipboard is updated after Ctrl+C
        thread::sleep(Duration::from_millis(50));

        let mut clipboard = Clipboard::new()?;
        let text = clipboard.get_text()?;
        Ok(text)
    }

    /// Set text to clipboard (for pasting)
    pub fn set_text(&self, text: &str) -> Result<()> {
        let mut clipboard = Clipboard::new()?;
        clipboard.set_text(text)?;
        Ok(())
    }

    /// Restore original clipboard content without dropping the guard
    pub fn restore(&self) -> Result<()> {
        if let Some(ref original) = self.original_text {
            let mut clipboard = Clipboard::new()?;
            clipboard.set_text(original)?;
        }
        Ok(())
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        // Restore original clipboard content
        if let Some(ref original) = self.original_text {
            if let Ok(mut clipboard) = Clipboard::new() {
                let _ = clipboard.set_text(original);
            }
        }
    }
}

impl Default for ClipboardGuard {
    fn default() -> Self {
        Self::new()
    }
}

/// Capture selected text using Ctrl+C with clipboard protection
pub fn capture_selected_text() -> Result<String> {
    use crate::input::send_ctrl_c;

    // Create guard to save and restore clipboard
    let guard = ClipboardGuard::new();

    // Send Ctrl+C to copy selected text
    send_ctrl_c();

    // Wait a bit for clipboard to update
    thread::sleep(Duration::from_millis(100));

    // Get the copied text
    let text = guard.get_text()?;

    // Check if we got the same text as before (nothing was selected)
    if let Some(ref original) = guard.original_text {
        if &text == original {
            anyhow::bail!("No text selected");
        }
    }

    // Don't restore clipboard yet - we'll do it after translation
    // Explicitly forget the guard to prevent Drop from running
    std::mem::forget(guard);

    Ok(text)
}

/// Paste text and restore original clipboard
pub fn paste_and_restore(text: &str, original: Option<String>) -> Result<()> {
    use crate::input::send_ctrl_v;

    let mut clipboard = Clipboard::new()?;

    // Set the translation result to clipboard
    clipboard.set_text(text)?;

    // Small delay before paste
    thread::sleep(Duration::from_millis(50));

    // Send Ctrl+V to paste
    send_ctrl_v();

    // Wait for paste to complete
    thread::sleep(Duration::from_millis(100));

    // Restore original clipboard content
    if let Some(original_text) = original {
        clipboard.set_text(&original_text)?;
    }

    Ok(())
}

/// Simple clipboard operations without protection
pub mod simple {
    use anyhow::Result;
    use arboard::Clipboard;

    pub fn get_text() -> Result<String> {
        let mut clipboard = Clipboard::new()?;
        Ok(clipboard.get_text()?)
    }

    pub fn set_text(text: &str) -> Result<()> {
        let mut clipboard = Clipboard::new()?;
        clipboard.set_text(text)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clipboard_guard_creation() {
        let guard = ClipboardGuard::new();
        // Just verify it doesn't panic
        drop(guard);
    }
}
