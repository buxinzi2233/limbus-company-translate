"""Real stdio JSON-RPC transport shared by release integration checks."""

import json
import selectors
import subprocess
import time


def rpc(process: subprocess.Popen, request_id: int, method: str, parameters: dict[str, object]) -> dict[str, object]:
    assert process.stdin and process.stdout
    process.stdin.write(json.dumps({"jsonrpc":"2.0", "id":request_id, "method":method, "params":parameters}) + "\n")
    process.stdin.flush()
    with selectors.DefaultSelector() as selector:
        selector.register(process.stdout, selectors.EVENT_READ)
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            if not selector.select(max(0, deadline - time.monotonic())):
                raise TimeoutError(f"MCP {method} request {request_id} exceeded 15 seconds")
            line = process.stdout.readline()
            if not line:
                raise RuntimeError(f"MCP stopped during {method}; exit={process.poll()}")
            result = json.loads(line)
            if result.get("id") == request_id:
                if "error" in result:
                    raise RuntimeError(f"MCP {method}: {result['error']}")
                return result["result"]
    raise TimeoutError(f"MCP {method} did not respond")


def stop(process: subprocess.Popen) -> None:
    process.terminate()
    process.wait(timeout=10)
