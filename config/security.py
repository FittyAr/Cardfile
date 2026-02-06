def normalize_allowed_ips(allowed_ips):
    if isinstance(allowed_ips, str):
        normalized = allowed_ips.replace("\n", ",")
        allowed_ips = [ip.strip() for ip in normalized.split(",") if ip.strip()]
    if not isinstance(allowed_ips, list):
        return ["0.0.0.0"]
    cleaned = [ip.strip() for ip in allowed_ips if isinstance(ip, str) and ip.strip()]
    return cleaned or ["0.0.0.0"]


def is_ip_allowed(allowed_ips, client_ip):
    allowed_ips = normalize_allowed_ips(allowed_ips)
    if "0.0.0.0" in allowed_ips:
        return True
    if client_ip is None:
        return False
    return client_ip in allowed_ips
