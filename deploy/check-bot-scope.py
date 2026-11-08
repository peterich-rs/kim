#!/usr/bin/env python3
"""Gate Chat rollout on the signed bot contract of every Royal instance."""

import hashlib
import hmac
import json
import secrets
import subprocess
import sys
import time
import urllib.request
import uuid


def check(containers):
    if len(containers) < 2:
        raise ValueError("both Royal container IDs are required")
    instances = json.loads(subprocess.check_output(["docker", "inspect", *containers]))
    networks = set.intersection(*(set(c["NetworkSettings"]["Networks"]) for c in instances))
    if not networks:
        raise ValueError("Royal instances have no shared network")
    network = sorted(networks)[0]
    scopes = []
    for instance in instances:
        env = dict(value.split("=", 1) for value in instance["Config"]["Env"] if "=" in value)
        secret = env.get("KIM_INTERNAL_HMAC_SECRET", "")
        if not secret:
            raise ValueError("Royal internal HMAC is not configured")
        address = instance["NetworkSettings"]["Networks"][network]["IPAddress"]
        path = "/internal/v1/bot/store-scope"
        timestamp = str(int(time.time()))
        nonce = secrets.token_hex(16)
        canonical = f"GET\n{path}\n{timestamp}\n{nonce}\n".encode()
        signature = hmac.new(secret.encode(), canonical, hashlib.sha256).hexdigest()
        request = urllib.request.Request(f"http://{address}:8080{path}", headers={
            "Accept": "application/json", "x-kim-timestamp": timestamp,
            "x-kim-nonce": nonce, "x-kim-signature": signature,
        })
        # Do not inherit a host HTTP proxy for an internal Docker address.
        with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(request, timeout=5) as response:
            scope = json.load(response)
        if scope.get("kind") not in ("memory", "postgres"):
            raise ValueError("Royal returned an unsupported message dataset")
        uuid.UUID(scope["id"])
        scopes.append((scope["kind"], scope["id"]))
    if len(set(scopes)) != 1:
        raise ValueError("Royal instances serve different message datasets")
    print(f"bot contract ready on {len(scopes)} Royal instances; scope={scopes[0][0]}:{scopes[0][1]}")


if __name__ == "__main__":
    try:
        check(sys.argv[1:])
    except Exception as error:
        # Never include inspect output or environment values in diagnostics.
        print(f"error: Royal bot contract gate failed ({type(error).__name__})", file=sys.stderr)
        sys.exit(1)
