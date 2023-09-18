use tracing_subscriber::{registry::LookupSpan, Layer};

pub struct LogConfig {
    is_prod: bool,
}

impl LogConfig {
    /// Create a new `LogConfig` instance.
    /// This config will use the Stackdriver logging layer if the
    /// `K_SERVICE` or `KUBERNETES_SERVICE_HOST` environment variables are set.
    /// Otherwise, it will use the stdout layer.
    ///
    /// How to use:
    /// ```
    /// use logging::LogConfig;
    /// use tracing_subscriber::{prelude::*, Registry};
    /// 
    /// let subscriber = Registry::default();
    /// let subscriber = subscriber.with(cfg.layer());
    /// tracing::subscriber::set_global_default(subscriber).unwrap();
    /// ```
    pub fn new() -> Self {
        let k_service = std::env::var("K_SERVICE");
        let kubernetes_service = std::env::var("KUBERNETES_SERVICE_HOST");
        Self {
            is_prod: k_service.is_ok() || kubernetes_service.is_ok(),
        }
    }

    pub fn layer<S>(&self) -> Box<dyn Layer<S> + Send + Sync + 'static>
    where
        S: tracing_core::Subscriber,
        for<'a> S: LookupSpan<'a>,
    {
        if self.is_prod {
            let stackdriver = tracing_stackdriver::layer();
            Box::new(stackdriver)
        } else {
            let stdout_log = tracing_subscriber::fmt::layer();
            Box::new(stdout_log)
        }
    }
}
