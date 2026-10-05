use super::TouchTarget;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct TouchState {
    active_id: Option<i32>,
    pub(super) target: TouchTarget,
    last_position: Option<(f64, f64)>,
}

impl TouchState {
    pub(super) fn begin(&mut self, id: i32, position: (f64, f64), target: TouchTarget) -> bool {
        if self.active_id.is_some() {
            return false;
        }
        self.active_id = Some(id);
        self.target = target;
        self.last_position = Some(position);
        true
    }

    pub(super) fn update_position(&mut self, id: i32, position: (f64, f64)) -> bool {
        if self.active_id != Some(id) {
            return false;
        }
        self.last_position = Some(position);
        true
    }

    pub(super) fn end(&mut self, id: i32) -> Option<((f64, f64), TouchTarget)> {
        if self.active_id != Some(id) {
            return None;
        }
        self.cancel()
    }

    pub(super) fn cancel(&mut self) -> Option<((f64, f64), TouchTarget)> {
        let end = self.last_position.map(|position| (position, self.target));
        self.clear();
        end
    }

    pub(super) fn set_target(&mut self, target: TouchTarget) {
        if self.active_id.is_some() {
            self.target = target;
        }
    }

    pub(super) fn clear(&mut self) {
        self.active_id = None;
        self.target = TouchTarget::None;
        self.last_position = None;
    }
}
