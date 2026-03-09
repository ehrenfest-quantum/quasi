import os
import cbor2
from afana.cbor import EhrenfestProgram

def load_ehrenfest_program():
    # Load all valid Ehrenfest programs from spec/valid
    programs = []
    for file in os.listdir("spec/valid"):
        if file.endswith(".cbor"):
            with open(os.path.join("spec/valid", file), 'rb') as f:
                program = cbor2.loads(f.read())
                programs.append(EhrenfestProgram::from_cbor(program))
    return programs