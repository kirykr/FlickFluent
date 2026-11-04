"""Start the sidecar from the runtime's Python: `python launcher.py --allow <dir> ...`.

The runtime runs with `-I`-style isolation in mind, so this file puts its own folder on the path
instead of relying on PYTHONPATH.
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from flickfluent_sidecar.main import main  # noqa: E402

if __name__ == "__main__":
    raise SystemExit(main())
