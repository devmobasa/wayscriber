#[derive(Debug, Clone)]
pub struct FormError {
    pub field: String,
    pub message: String,
}

impl FormError {
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}
