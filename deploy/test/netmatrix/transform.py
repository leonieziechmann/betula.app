#!/usr/bin/env python3
"""What deploy/test/netmatrix.sh deploys: a stack file as "docker stack config" rendered it, with
everything the networks do not depend on swapped out.

  transform.py STACK WORKDIR < rendered.yml > deployed.yml

Kept exactly: the services, their networks, the networks of the file, the Traefik labels. Real
images stay for Traefik, its socket proxy, Prometheus, the scraper and Alloy (the metrics
entrypoint's allow list, the remote writes, Alloy's textfile collectors); every other service
becomes a stand-in (netmatrix/standin.sh on alpine) that answers on its ports. Bind mounts of
/opt/betula/config come from WORKDIR/config (a copy), secrets go, logging goes (the journald driver
needs a journal), every service gets endpoint_mode dnsrr (no virtual address: on some hosts, this
sandbox among them, IPVS does not work, and the probes use task addresses anyway), and Traefik's
ports move to 18080 and 18443.
"""
import sys

import yaml

STANDIN_PORTS = {
    "placeholder_web": "8080",
    "monitoring_grafana": "3000",
    "monitoring_loki": "3100",
    "cortex_a": "8100",
    "cortex_b": "8100",
}
REAL = {"edge_traefik", "edge_socket-proxy", "monitoring_prometheus", "monitoring_scraper", "monitoring_alloy"}
PORT_MAP = {80: 18080, 443: 18443}


def ports_of(full):
    if full in STANDIN_PORTS:
        return STANDIN_PORTS[full]
    if full.endswith("_radix"):
        return "8090"
    if full.endswith("_folia"):
        return "8080"
    raise SystemExit(f"transform.py: no stand-in for {full}")


def remap_source(source, work):
    if source.startswith("/opt/betula/config/"):
        return work + "/config/" + source[len("/opt/betula/config/"):]
    if source == "/var/log/journal":
        return work + "/journal"
    return source


def escape(value):
    """docker stack config has substituted every variable and turned "$$" into "$"; deployed
    again, the file would be substituted a second time. Every "$" left is a literal one."""
    if isinstance(value, str):
        return value.replace("$", "$$")
    if isinstance(value, list):
        return [escape(v) for v in value]
    if isinstance(value, dict):
        return {k: escape(v) for k, v in value.items()}
    return value


def add_label(deploy):
    labels = deploy.get("labels") or {}
    if isinstance(labels, list):
        labels.append("app.betula.netmatrix=1")
    else:
        labels["app.betula.netmatrix"] = "1"
    deploy["labels"] = labels


def main():
    stack, work = sys.argv[1], sys.argv[2]
    doc = yaml.safe_load(sys.stdin)
    doc.pop("secrets", None)
    for name, svc in (doc.get("services") or {}).items():
        full = f"{stack}_{name}"
        deploy = svc.setdefault("deploy", {})
        deploy["endpoint_mode"] = "dnsrr"
        add_label(deploy)
        svc.pop("logging", None)
        svc.pop("secrets", None)
        if full in REAL:
            for mount in svc.get("volumes") or []:
                if isinstance(mount, dict) and mount.get("type") == "bind":
                    mount["source"] = remap_source(mount["source"], work)
                    # rslave needs a host whose / is a shared mount (systemd's default, not every
                    # container host's); the harness's stats files sit on a mount that is there
                    # before Alloy starts.
                    (mount.get("bind") or {}).pop("propagation", None)
            for port in svc.get("ports") or []:
                if isinstance(port, dict) and port.get("published") in PORT_MAP:
                    port["published"] = PORT_MAP[port["published"]]
            continue
        svc["image"] = "alpine:3.22"
        svc.pop("entrypoint", None)
        svc.pop("user", None)
        svc.pop("cap_add", None)
        svc["command"] = ["sh", "/standin/standin.sh"]
        svc["environment"] = {"NAME": full, "LISTEN": ports_of(full)}
        svc["volumes"] = [{"type": "bind", "source": work + "/standin", "target": "/standin", "read_only": True}]
        svc["healthcheck"] = {"disable": True}
        svc["stop_grace_period"] = "2s"
        deploy.pop("resources", None)
        deploy.pop("update_config", None)
        deploy.pop("rollback_config", None)
    services = (doc.get("services") or {}).values()
    if not any(isinstance(m, dict) and m.get("type") == "volume" for s in services for m in (s.get("volumes") or [])):
        doc.pop("volumes", None)
    yaml.safe_dump(escape(doc), sys.stdout, sort_keys=False)


if __name__ == "__main__":
    main()
