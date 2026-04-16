use afana::cbor::{from_cbor, CborError};
use ciborium::cbor;

#[test]
fn test_non_numeric_coefficient() {
    // Build a CBOR value that represents a Hamiltonian term with a string coefficient
    let cbor_value = cbor!({
        "version" => 1,
        "system" => {
            "n_qubits" => 2,
        },
        "hamiltonian" => {
            "terms" => [
                {
                    "coefficient" => "not a number",   // This is a string!
                    "paulis" => [
                        { "qubit" => 0, "axis" => 1 }, // X on qubit 0
                    ],
                }
            ],
            "constant_offset" => 0.0,
        },
        "evolution" => {
            "total_us" => 1.0,
            "steps" => 10,
            "dt_us" => 0.1,
        },
        "observables" => [{"type" => "SZ", "qubit" => 0}],
        "noise" => {
            "t1_us" => 100.0,
            "t2_us" => 50.0,
        },
    }).unwrap();

    let mut bytes = Vec::new();
    ciborium::into_writer(&cbor_value, &mut bytes).unwrap();

    match from_cbor(&bytes) {
        Err(CborError::Decode(_)) => {} // expected
        other => panic!("Expected CborError::Decode, got {:?}", other),
    }
}