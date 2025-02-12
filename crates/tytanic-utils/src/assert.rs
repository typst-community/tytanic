/// Statically assert that `T` is [`Send`].
pub fn send<T: Send>() {}

/// Statically assert that `T` is [`Sync`].
pub fn sync<T: Sync>() {}
