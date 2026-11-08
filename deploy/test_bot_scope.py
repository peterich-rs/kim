"""The rollout gate must authenticate every replica and reject mixed datasets."""
import hashlib
import hmac
import importlib.util
import io
import json
from pathlib import Path
import unittest
from unittest.mock import patch
import urllib.error

spec = importlib.util.spec_from_file_location("scope_gate", Path(__file__).with_name("check-bot-scope.py"))
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


class ScopeGateTests(unittest.TestCase):
    def run_gate(self, scopes):
        instances = [{"Config": {"Env": ["KIM_INTERNAL_HMAC_SECRET=unit-test-only"]},
                      "NetworkSettings": {"Networks": {"kim_default": {"IPAddress": f"172.20.0.{i+2}"}}}}
                     for i in range(2)]
        requests = []

        class Opener:
            def open(self, request, timeout):
                requests.append(request)
                scope = scopes[len(requests)-1]
                if isinstance(scope, Exception):
                    raise scope
                timestamp = request.get_header("X-kim-timestamp")
                assert timestamp == "1794124800"
                nonce = request.get_header("X-kim-nonce")
                canonical = f"GET\n/internal/v1/bot/store-scope\n{timestamp}\n{nonce}\n".encode()
                expected = hmac.new(b"unit-test-only", canonical, hashlib.sha256).hexdigest()
                assert request.get_header("X-kim-signature") == expected
                assert timeout == 5
                return io.BytesIO(json.dumps(scope).encode())

        with patch.object(gate.subprocess, "check_output", return_value=json.dumps(instances).encode()), \
             patch.object(gate.time, "time", return_value=1794124800.75), \
             patch.object(gate.urllib.request, "build_opener", return_value=Opener()):
            gate.check(["royal-a", "royal-b"])
        self.assertEqual(len(requests), 2)

    def test_both_replicas_authenticate_and_prove_same_dataset(self):
        scope = {"kind": "postgres", "id": "8670d359-e5f5-4258-a92b-167a760623d4"}
        self.run_gate([scope, scope])

    def test_independent_memory_replicas_block_rollout(self):
        with self.assertRaisesRegex(ValueError, "different message datasets"):
            self.run_gate([{"kind": "memory", "id": "8670d359-e5f5-4258-a92b-167a760623d4"},
                           {"kind": "memory", "id": "e170d359-e5f5-4258-a92b-167a760623d4"}])

    def test_old_second_replica_blocks_rollout(self):
        with self.assertRaises(urllib.error.HTTPError):
            self.run_gate([{"kind": "postgres", "id": "8670d359-e5f5-4258-a92b-167a760623d4"},
                           urllib.error.HTTPError("http://royal-2", 404, "missing", {}, None)])

    def test_invalid_scope_is_not_accepted(self):
        with self.assertRaises(ValueError):
            self.run_gate([{"kind": "postgres", "id": "not-a-uuid"}])


if __name__ == "__main__":
    unittest.main()
