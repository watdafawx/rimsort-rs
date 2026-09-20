"""Run RimSort's real Python metadata + sort code on a real install and dump the sorted order.

Usage: debug/pyvenv/Scripts/python.exe tests/golden/py_sort.py <instance-settings.json> <out.json>
Reads game/local/workshop/config paths from the RimSort-style settings file (current instance) and the
community/user rules from RimSort's dbs folder. Never writes ModsConfig.xml.
"""
import json
import os
import sys
from pathlib import Path
from unittest.mock import MagicMock

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "reference-python"))
for name in ("PySide6", "PySide6.QtCore", "PySide6.QtWidgets", "PySide6.QtGui", "app.views.dialogue"):
    sys.modules[name] = MagicMock()

import importlib


def import_all():
    """Import the reference modules, mocking third-party packages that aren't installed (GUI, steam...)."""
    for _ in range(40):
        try:
            return (
                importlib.import_module("app.controllers.sort_controller"),
                importlib.import_module("app.models.metadata.metadata_factory"),
                importlib.import_module("app.models.metadata.metadata_structure"),
                importlib.import_module("app.utils.constants"),
            )
        except ModuleNotFoundError as e:
            if not e.name or e.name.startswith("app"):
                raise
            sys.modules[e.name] = MagicMock()
    raise RuntimeError("too many missing modules")


from loguru import logger  # noqa: E402

logger.remove()
sort_controller, factory, structure, constants = import_all()
Sorter = sort_controller.Sorter
create_listed_mod_from_path = factory.create_listed_mod_from_path
create_rules_from_external_rules = factory.create_rules_from_external_rules
read_mods_config = factory.read_mods_config
read_rules_db = factory.read_rules_db
AboutXmlMod = structure.AboutXmlMod
CompiledDependencyData = structure.CompiledDependencyData
SortMethod = constants.SortMethod

settings = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
inst = settings["instances"][settings["current_instance"]]
game, local, workshop = Path(inst["game_folder"]), Path(inst["local_folder"]), Path(inst["workshop_folder"])
config = Path(os.environ.get("MODSCONFIG", Path(inst["config_folder"]) / "ModsConfig.xml"))
dbs = Path(os.environ["LOCALAPPDATA"]) / "RimSort" / "dbs"
version = (game / "Version.txt").read_text().strip()

community = read_rules_db(dbs / "Community-Rules-Database" / "communityRules.json")
user = read_rules_db(dbs / "userRules.json")

mods = {}
for root in (game / "Data", local, workshop):
    if not root.is_dir():
        continue
    for d in sorted(p for p in root.iterdir() if p.is_dir()):
        valid, mod = create_listed_mod_from_path(d, version, local, game, workshop, True, True)
        if isinstance(mod, AboutXmlMod):
            pid = str(mod.package_id)
            if user and pid in user.rules:
                mod.user_rules = create_rules_from_external_rules(user.rules[pid])
            if community and pid in community.rules:
                mod.community_rules = create_rules_from_external_rules(community.rules[pid])
        mods[mod.uuid] = mod

by_pid = {}
for path, m in mods.items():
    if isinstance(m, AboutXmlMod):
        by_pid.setdefault(str(m.package_id), []).append(path)

cfg = read_mods_config(config)
active_paths = []
for raw in cfg.activeMods:
    pid = str(raw).lower().removesuffix("_steam")
    if pid in by_pid:
        active_paths.append(sorted(by_pid[pid])[0])

compiled = CompiledDependencyData.build(mods, use_moddependencies_as_loadTheseBefore=False, use_alternative_package_ids=True)
ok, sorted_paths = Sorter(SortMethod.TOPOLOGICAL, compiled, mods, set(active_paths)).sort()

out = {
    "ok": ok,
    "before": [str(mods[p].package_id) for p in active_paths],
    "sorted": [str(mods[p].package_id) for p in sorted_paths],
}
Path(sys.argv[2]).write_text(json.dumps(out, indent=1))
print(f"ok={ok} active={len(active_paths)} sorted={len(sorted_paths)}")
