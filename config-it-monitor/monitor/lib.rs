//!
//!
//!
pub mod server;
mod session;
pub mod trace {
    use tracing::span;

    pub struct Subscriber {}

    impl tracing::Subscriber for Subscriber {
        fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
            todo!()
        }

        fn new_span(&self, span: &span::Attributes<'_>) -> span::Id {
            todo!()
        }

        fn record(&self, span: &span::Id, values: &span::Record<'_>) {
            todo!()
        }

        fn record_follows_from(&self, span: &span::Id, follows: &span::Id) {
            todo!()
        }

        fn event(&self, event: &tracing::Event<'_>) {
            todo!()
        }

        fn enter(&self, span: &span::Id) {
            todo!()
        }

        fn exit(&self, span: &span::Id) {
            todo!()
        }
    }
}
