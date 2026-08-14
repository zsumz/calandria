//! Automatic effect rollback for typed failure and panic unwinding.

use core::mem;

use calandria::Retained;

use super::ActionContext;

impl<E: Retained, O: Retained> Drop for ActionContext<'_, E, O> {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        let inserted = mem::take(&mut self.inserted);
        for (token, _) in inserted {
            let removed = self
                .timeline
                .cancel(token)
                .unwrap_or_else(|| panic!("transactional insertion must roll back"));
            drop(removed);
        }
        let removed = mem::take(&mut self.removed);
        for (token, event) in removed {
            self.timeline.restore(token, event);
        }
    }
}
