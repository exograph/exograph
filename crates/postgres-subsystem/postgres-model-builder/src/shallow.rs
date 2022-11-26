use core_plugin_interface::core_model::mapped_arena::SerializableSlabIndex;

/// A trait that allows a default-like value during shallow building
///
/// It is expected that the shallow value will be replaced with a real value
/// during the expansion phase.
pub trait Shallow {
    fn shallow() -> Self;
}

impl<T> Shallow for SerializableSlabIndex<T> {
    fn shallow() -> Self {
        // Use an impossible index to make sure we don't accidentally use this (or if we use, it will panic)
        SerializableSlabIndex::from_idx(usize::MAX)
    }
}
