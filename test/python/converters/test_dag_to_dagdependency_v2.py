# This code is part of Qiskit.
#
# (C) Copyright IBM 2020.
#
# This code is licensed under the Apache License, Version 2.0. You may
# obtain a copy of this license in the LICENSE.txt file in the root directory
# of this source tree or at http://www.apache.org/licenses/LICENSE-2.0.
#
# Any modifications or derivative works of this code must retain this
# copyright notice, and modified files need to carry a notice indicating
# that they have been altered from the originals.

"""Test for the converter dag dependency to dag circuit and
dag circuit to dag dependency."""

import unittest

from qiskit.converters.circuit_to_dag import circuit_to_dag
from qiskit.converters.dag_to_dagdependency_v2 import _dag_to_dagdependency_v2
from qiskit.converters.dagdependency_to_dag import dagdependency_to_dag
from qiskit import QuantumRegister, ClassicalRegister, QuantumCircuit
from test import QiskitTestCase  # pylint: disable=wrong-import-order


class TestCircuitToDagDependencyV2(QiskitTestCase):
    """Test DAGCircuit to DAGDependencyV2."""

    def test_circuit_and_dag_dependency(self):
        """Check convert to dag dependency and back"""
        qr = QuantumRegister(3)
        cr = ClassicalRegister(3)
        circuit_in = QuantumCircuit(qr, cr)
        circuit_in.h(qr[0])
        circuit_in.h(qr[1])
        circuit_in.measure(qr[0], cr[0])
        circuit_in.measure(qr[1], cr[1])
        circuit_in.measure(qr[0], cr[0])
        circuit_in.measure(qr[1], cr[1])
        circuit_in.measure(qr[2], cr[2])
        dag_in = circuit_to_dag(circuit_in)

        dag_dependency = _dag_to_dagdependency_v2(dag_in)
        dag_out = dagdependency_to_dag(dag_dependency)

        self.assertEqual(dag_out, dag_in)

    def test_metadata(self):
        """Test circuit metadata is preservered through conversion."""
        meta_dict = {"experiment_id": "1234", "execution_number": 4}
        qr = QuantumRegister(2)
        circuit_in = QuantumCircuit(qr, metadata=meta_dict)
        circuit_in.h(qr[0])
        circuit_in.cx(qr[0], qr[1])
        circuit_in.measure_all()
        dag = circuit_to_dag(circuit_in)
        self.assertEqual(dag.metadata, meta_dict)
        dag_dependency = _dag_to_dagdependency_v2(dag)
        self.assertEqual(dag_dependency.metadata, meta_dict)
        dag_out = dagdependency_to_dag(dag_dependency)
        self.assertEqual(dag_out.metadata, meta_dict)


if __name__ == "__main__":
    unittest.main(verbosity=2)
