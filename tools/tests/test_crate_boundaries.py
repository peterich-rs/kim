import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("boundaries", Path(__file__).parents[1] / "check_crate_boundaries.py")
boundaries = importlib.util.module_from_spec(spec)
spec.loader.exec_module(boundaries)


class ModernCargoGraph(unittest.TestCase):
    def test_transitive_normal_alias_cannot_hide_process_dependency(self):
        names = ["royal", "helper", "chat"]
        ids = [f"path+file:///repo/{name}#0.1.0" for name in names]
        data = {
            "packages": [{"name": name, "id": ident} for name, ident in zip(names, ids)],
            "resolve": {"nodes": [
                {"id": ids[0], "deps": [{"name": "aliased_helper", "pkg": ids[1], "dep_kinds": [{"kind": None}]}]},
                {"id": ids[1], "deps": [{"name": "aliased_chat", "pkg": ids[2], "dep_kinds": [{"kind": None}]}]},
                {"id": ids[2], "deps": []},
            ]},
        }
        nodes = boundaries.graph_nodes(data)
        reached = boundaries.closure(nodes, "royal", {"normal", "build"})
        self.assertIn("chat", reached)
        self.assertTrue(boundaries.forbidden_hit("royal", "chat"))
        data["resolve"]["nodes"][0]["deps"][0]["dep_kinds"][0]["kind"] = "dev"
        nodes = boundaries.graph_nodes(data)
        self.assertNotIn("chat", boundaries.closure(nodes, "royal", {"normal", "build"}))
        self.assertIn("chat", boundaries.closure(nodes, "royal", {"normal", "build", "dev"}))


if __name__ == "__main__":
    unittest.main()
