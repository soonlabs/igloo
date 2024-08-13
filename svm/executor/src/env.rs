use std::sync::{Arc, RwLock};

use crate::mock::fork_graph::MockForkGraph;
use solana_bpf_loader_program::syscalls::{
    SyscallAbort, SyscallGetClockSysvar, SyscallInvokeSignedRust, SyscallLog, SyscallMemcpy,
    SyscallMemset, SyscallSetReturnData,
};
use solana_compute_budget::compute_budget::ComputeBudget;
use solana_program_runtime::{
    invoke_context::InvokeContext,
    loaded_programs::{ProgramCache, ProgramRuntimeEnvironments},
    solana_rbpf::{
        program::{BuiltinFunction, BuiltinProgram, FunctionRegistry},
        vm::Config,
    },
};

pub const DEPLOYMENT_SLOT: u64 = 0;
pub const DEPLOYMENT_EPOCH: u64 = 0;

pub fn create_custom_environment<'a>() -> BuiltinProgram<InvokeContext<'a>> {
    let compute_budget = ComputeBudget::default();
    let vm_config = Config {
        max_call_depth: compute_budget.max_call_depth,
        stack_frame_size: compute_budget.stack_frame_size,
        enable_address_translation: true,
        enable_stack_frame_gaps: true,
        instruction_meter_checkpoint_distance: 10000,
        enable_instruction_meter: true,
        enable_instruction_tracing: true,
        enable_symbol_and_section_labels: true,
        reject_broken_elfs: true,
        noop_instruction_rate: 256,
        sanitize_user_provided_values: true,
        external_internal_function_hash_collision: false,
        reject_callx_r10: false,
        enable_sbpf_v1: true,
        enable_sbpf_v2: false,
        optimize_rodata: false,
        aligned_memory_mapping: true,
        new_elf_parser: false,
    };

    // These functions are system calls the compile contract calls during execution, so they
    // need to be registered.
    let mut function_registry = FunctionRegistry::<BuiltinFunction<InvokeContext>>::default();
    function_registry
        .register_function_hashed(*b"abort", SyscallAbort::vm)
        .expect("Registration failed");
    function_registry
        .register_function_hashed(*b"sol_log_", SyscallLog::vm)
        .expect("Registration failed");
    function_registry
        .register_function_hashed(*b"sol_memcpy_", SyscallMemcpy::vm)
        .expect("Registration failed");
    function_registry
        .register_function_hashed(*b"sol_memset_", SyscallMemset::vm)
        .expect("Registration failed");

    function_registry
        .register_function_hashed(*b"sol_invoke_signed_rust", SyscallInvokeSignedRust::vm)
        .expect("Registration failed");

    function_registry
        .register_function_hashed(*b"sol_set_return_data", SyscallSetReturnData::vm)
        .expect("Registration failed");

    function_registry
        .register_function_hashed(*b"sol_get_clock_sysvar", SyscallGetClockSysvar::vm)
        .expect("Registration failed");

    BuiltinProgram::new_loader(vm_config, function_registry)
}

pub fn create_executable_environment(
    fork_graph: Arc<RwLock<MockForkGraph>>,
    program_cache: &mut ProgramCache<MockForkGraph>,
) {
    program_cache.environments = ProgramRuntimeEnvironments {
        program_runtime_v1: Arc::new(create_custom_environment()),
        // We are not using program runtime v2
        program_runtime_v2: Arc::new(BuiltinProgram::new_loader(
            Config::default(),
            FunctionRegistry::default(),
        )),
    };

    program_cache.fork_graph = Some(Arc::downgrade(&fork_graph));
}
