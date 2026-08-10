pub const IMAGE_STATUS_PENDING: &str = "pending";
pub const IMAGE_STATUS_PROCESSING: &str = "processing";
pub const IMAGE_STATUS_COMPLETED: &str = "completed";
pub const IMAGE_STATUS_FAILED: &str = "failed";

pub fn is_pending_status(status: &str) -> bool {
    matches!(status, IMAGE_STATUS_PENDING | IMAGE_STATUS_PROCESSING)
}
