//! Main-crate forwarding surface for the private broker package.

pub(crate) use wayscriber_process_broker::{
    BROKER_BUSY, BrokerChild, BrokerOutput, HelperKind, HelperLifetime, ProcessBroker,
    STDOUT_CAP_EXCEEDED, current, max_publish_bytes,
};

#[cfg(not(test))]
pub(crate) use wayscriber_process_broker::start_for_runtime;
#[cfg(test)]
pub(crate) use wayscriber_process_broker::start_for_test as start_for_runtime;
