#![no_main]
use libfuzzer_sys::fuzz_target;
use fog_pack::NoSchema;

fuzz_target!(|data: &[u8]| {
    use fog_pack::validator;
    let _ = NoSchema::decode_doc(Vec::from(data));
});
