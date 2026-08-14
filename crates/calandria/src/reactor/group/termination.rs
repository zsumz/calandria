//! Bounded group-wide fail-safe termination observations.

use std::sync::Arc;

use crate::{
    ReactorTermination,
    reactor::{ReactorControl, control::PublishedTermination},
};

use super::{AdmissionCloser, ReactorId};

/// Exact per-reactor results of one group termination request.
#[must_use = "group termination can carry per-reactor wake failures"]
#[derive(Debug)]
pub struct ReactorGroupTermination {
    reactors: Box<[ReactorTermination]>,
}

impl ReactorGroupTermination {
    /// Returns the number of reactors observed by this request.
    pub const fn len(&self) -> usize {
        self.reactors.len()
    }

    /// Returns whether the group contained no reactors.
    pub const fn is_empty(&self) -> bool {
        self.reactors.is_empty()
    }

    /// Returns the result for one valid reactor identity.
    pub fn get(&self, reactor: ReactorId) -> Option<&ReactorTermination> {
        reactor
            .position()
            .and_then(|position| self.reactors.get(position))
    }

    /// Iterates results in stable reactor identity order.
    pub fn iter(&self) -> impl Iterator<Item = (ReactorId, &ReactorTermination)> {
        self.reactors
            .iter()
            .enumerate()
            .map(|(index, result)| (ReactorId::from_position(index), result))
    }
}

pub(super) struct GroupControl<T> {
    controls: Arc<[ReactorControl]>,
    admission: AdmissionCloser<T>,
}

impl<T> GroupControl<T> {
    pub(super) fn new(controls: Vec<ReactorControl>, admission: AdmissionCloser<T>) -> Self {
        Self {
            controls: controls.into(),
            admission,
        }
    }

    pub(super) fn request_termination(&self) -> ReactorGroupTermination {
        self.admission.close();
        let published = self
            .controls
            .iter()
            .map(ReactorControl::publish_termination)
            .collect::<Vec<_>>();
        ReactorGroupTermination {
            reactors: published
                .into_iter()
                .map(PublishedTermination::complete)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        }
    }

    pub(super) fn admission(&self) -> AdmissionCloser<T> {
        self.admission.clone()
    }
}

impl<T> Clone for GroupControl<T> {
    fn clone(&self) -> Self {
        Self {
            controls: Arc::clone(&self.controls),
            admission: self.admission.clone(),
        }
    }
}
