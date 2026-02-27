OPENQASM 2.0;
include "qelib1.inc";

qreg q[2];
creg c[2];

// Shallow ansatz for H2-like toy benchmark
ry(1.0471975512) q[0];
ry(0.7853981634) q[1];
cx q[0],q[1];
rz(0.6283185307) q[1];
cx q[0],q[1];

measure q -> c;
