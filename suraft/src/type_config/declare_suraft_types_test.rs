//! Test the `declare_suraft_types` macro with default values

use crate::declare_suraft_types;
use crate::impls::TokioRuntime;

declare_suraft_types!(
    All:
        /// This is AppData
        AppData = (),
        #[allow(dead_code)]
        #[allow(dead_code)]
        AsyncRuntime = TokioRuntime,
        Responder = crate::impls::OneshotResponder<Self>,
);

declare_suraft_types!(
    WithoutD:
        AsyncRuntime = TokioRuntime,
);

declare_suraft_types!(
    WithoutR:
        AppData = (),
        AsyncRuntime = TokioRuntime,
);

declare_suraft_types!(EmptyWithColon:);

declare_suraft_types!(Empty);
