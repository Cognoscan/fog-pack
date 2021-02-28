use crate::{
    element::Element,
    error::{Error, Result},
    MAX_DEPTH,
};

#[derive(Clone, Debug)]
pub struct DepthTracker {
    tracking: Vec<u32>,
}

impl DepthTracker {
    /// Create a new depth tracker
    pub fn new() -> Self {
        Self {
            tracking: Vec::new(),
        }
    }

    /// Update the depth tracker on each new element to serialize.
    pub fn update_elem(&mut self, elem: &Element) -> Result<()> {
        // Subtract from count for next element
        if let Some(v) = self.tracking.last_mut() {
            *v -= 1;
        }

        // Increase nest depth if this is a nesting element
        match elem {
            Element::Map(len) => self.tracking.push(2 * (*len as u32)), // 2 elements per map item
            Element::Array(len) => self.tracking.push(*len as u32),
            _ => (),
        }

        // Check to see if we hit the nesting limit
        if self.tracking.len() > MAX_DEPTH {
            return Err(Error::ParseLimit("Depth limit exceeded".to_string()));
        }

        self.purge_zeros();
        Ok(())
    }

    /// Drop any depth tracking elements that have hit zero
    pub fn purge_zeros(&mut self) {
        loop {
            match self.tracking.last() {
                Some(v) if *v == 0 => {
                    self.tracking.pop();
                }
                _ => break,
            }
        }
    }

    /// Drop a depth before we've seen enough elements. This can be used by map/seq serializers
    /// that didn't know their total length ahead of time. This way, they can put in a
    /// maximally-sized map/array element, then run through the depth tracker as normal, calling
    /// this when done.
    pub fn early_end(&mut self) {
        self.tracking.pop();
        self.purge_zeros();
    }
}
