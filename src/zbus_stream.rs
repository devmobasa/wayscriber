use std::{future::poll_fn, pin::Pin};

use zbus::export::futures_core::Stream;

pub(crate) async fn next<S>(stream: &mut S) -> Option<S::Item>
where
    S: Stream + Unpin,
{
    poll_fn(|cx| Pin::new(&mut *stream).poll_next(cx)).await
}
