//! Emergency stop — global panic button for instant device shutdown.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::broadcast;
use tracing::warn;

/// Global emergency stop controller.
///
/// Thread-safe, lock-free. Fires broadcast notifications to all subscribers
/// when triggered. Can be cloned and shared across tasks.
#[derive(Clone)]
pub struct EmergencyStop {
    triggered: Arc<AtomicBool>,
    armed: Arc<AtomicBool>,
    trigger_count: Arc<AtomicU64>,
    tx: broadcast::Sender<String>,
}

impl EmergencyStop {
    #[must_use]
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(16);
        Self {
            triggered: Arc::new(AtomicBool::new(false)),
            armed: Arc::new(AtomicBool::new(false)),
            trigger_count: Arc::new(AtomicU64::new(0)),
            tx,
        }
    }

    /// Arm the emergency stop.
    pub fn arm(&self) {
        self.triggered.store(false, Ordering::SeqCst);
        self.armed.store(true, Ordering::SeqCst);
    }

    /// Disarm the emergency stop.
    pub fn disarm(&self) {
        self.armed.store(false, Ordering::SeqCst);
    }

    /// Trigger emergency stop. Broadcasts reason to all subscribers.
    pub fn trigger(&self, reason: &str) {
        if self
            .triggered
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return; // Already triggered
        }

        self.trigger_count.fetch_add(1, Ordering::Relaxed);
        warn!(reason, "EMERGENCY STOP triggered");
        let _ = self.tx.send(reason.to_string());
    }

    /// Reset after an emergency stop.
    pub fn reset(&self) {
        self.triggered.store(false, Ordering::SeqCst);
    }

    /// Subscribe to emergency stop events.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    #[must_use]
    pub fn is_triggered(&self) -> bool {
        self.triggered.load(Ordering::SeqCst)
    }

    #[must_use]
    pub fn is_armed(&self) -> bool {
        self.armed.load(Ordering::SeqCst) && !self.is_triggered()
    }

    #[must_use]
    pub fn trigger_count(&self) -> u64 {
        self.trigger_count.load(Ordering::Relaxed)
    }
}

impl Default for EmergencyStop {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_and_reset() {
        let estop = EmergencyStop::new();
        estop.arm();
        assert!(estop.is_armed());
        assert!(!estop.is_triggered());

        estop.trigger("test");
        assert!(estop.is_triggered());
        assert!(!estop.is_armed());
        assert_eq!(estop.trigger_count(), 1);

        // Double trigger is no-op
        estop.trigger("test2");
        assert_eq!(estop.trigger_count(), 1);

        estop.reset();
        assert!(!estop.is_triggered());
    }

    #[tokio::test]
    async fn broadcast_notification() {
        let estop = EmergencyStop::new();
        let mut rx = estop.subscribe();
        estop.arm();

        estop.trigger("panic");
        let reason = rx.recv().await.unwrap();
        assert_eq!(reason, "panic");
    }

    #[test]
    fn clone_shares_state() {
        let estop = EmergencyStop::new();
        let estop2 = estop.clone();
        estop.arm();
        estop.trigger("shared");
        assert!(estop2.is_triggered());
    }

    #[test]
    fn not_armed_initially() {
        let estop = EmergencyStop::new();
        assert!(!estop.is_armed());
        assert!(!estop.is_triggered());
        assert_eq!(estop.trigger_count(), 0);
    }

    #[test]
    fn disarm() {
        let estop = EmergencyStop::new();
        estop.arm();
        assert!(estop.is_armed());
        estop.disarm();
        assert!(!estop.is_armed());
    }

    #[test]
    fn trigger_count_after_reset_and_retrigger() {
        let estop = EmergencyStop::new();
        estop.arm();
        estop.trigger("first");
        assert_eq!(estop.trigger_count(), 1);
        estop.reset();
        estop.trigger("second");
        assert_eq!(estop.trigger_count(), 2);
    }

    #[test]
    fn is_armed_false_after_trigger() {
        let estop = EmergencyStop::new();
        estop.arm();
        assert!(estop.is_armed());
        estop.trigger("boom");
        // is_armed = armed && !triggered → false
        assert!(!estop.is_armed());
    }

    #[test]
    fn default_trait() {
        let estop = EmergencyStop::default();
        assert!(!estop.is_armed());
        assert!(!estop.is_triggered());
    }

    #[tokio::test]
    async fn multiple_subscribers() {
        let estop = EmergencyStop::new();
        let mut rx1 = estop.subscribe();
        let mut rx2 = estop.subscribe();
        estop.arm();
        estop.trigger("multi");
        assert_eq!(rx1.recv().await.unwrap(), "multi");
        assert_eq!(rx2.recv().await.unwrap(), "multi");
    }

    #[test]
    fn arm_clears_triggered() {
        let estop = EmergencyStop::new();
        estop.arm();
        estop.trigger("first");
        assert!(estop.is_triggered());
        // Re-arm should clear triggered
        estop.arm();
        assert!(!estop.is_triggered());
        assert!(estop.is_armed());
    }
}
