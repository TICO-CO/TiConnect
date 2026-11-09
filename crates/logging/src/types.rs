use tracing_subscriber::{EnvFilter, Layer, Registry, layer::Layered, reload};

pub type FilterReload = reload::Layer<EnvFilter, Registry>;
pub type FilterHandle = reload::Handle<EnvFilter, Registry>;
pub type BaseSubscriber = Layered<FilterReload, Registry>;

pub type OutputLayer = Box<dyn Layer<BaseSubscriber> + Send + Sync + 'static>;
pub type OutputHandle = reload::Handle<OutputLayer, BaseSubscriber>;
