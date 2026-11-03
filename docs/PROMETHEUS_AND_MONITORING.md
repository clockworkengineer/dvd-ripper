# Prometheus Metrics & Health Monitoring Guide - `dvd-ripper`

`dvd-ripper` features a native, zero-dependency Prometheus exposition format endpoint and container-ready health probes to support enterprise monitoring, homelab observability (Grafana / Prometheus), and headless NAS integration (Unraid, TrueNAS, Kubernetes).

---

## 1. Endpoints Overview

| Endpoint | HTTP Method | Content-Type | Auth Required? | Purpose |
|---|---|---|---|---|
| `/metrics` | `GET` | `text/plain; version=0.0.4; charset=utf-8` | No (Public) | Standard Prometheus text metrics exposition. |
| `/api/health` | `GET` | `application/json` | No (Public) | Liveness & readiness probe for container orchestrators. |
| `/api/status` | `GET` | `application/json` | No (Public) | High-level appliance state for dashboard widgets. |
| `/api/pool/status`| `GET` | `application/json` | Yes (if API key set) | Detailed per-drive multi-drive pool telemetry. |

---

## 2. Metric Catalog & Semantics

The embedded web server exposes the following metrics at `http://<host>:8080/metrics`:

```text
# HELP dvd_ripper_completed_rips_total Total number of successful DVD ripping jobs
# TYPE dvd_ripper_completed_rips_total counter
dvd_ripper_completed_rips_total 14

# HELP dvd_ripper_failed_rips_total Total number of failed DVD ripping jobs
# TYPE dvd_ripper_failed_rips_total counter
dvd_ripper_failed_rips_total 1

# HELP dvd_ripper_active_jobs Current number of active DVD ripping processes
# TYPE dvd_ripper_active_jobs gauge
dvd_ripper_active_jobs 2

# HELP dvd_ripper_queued_jobs Current number of pending ripping jobs in queue
# TYPE dvd_ripper_queued_jobs gauge
dvd_ripper_queued_jobs 3

# HELP dvd_ripper_progress_percent Current ripping job progress percentage
# TYPE dvd_ripper_progress_percent gauge
dvd_ripper_progress_percent 72.8
```

### Metric Details:
1. **`dvd_ripper_completed_rips_total`** (Counter):
   - Increments monotonically whenever a title extraction, TV batch, or ISO imaging process successfully finishes and passes validation checks.
2. **`dvd_ripper_failed_rips_total`** (Counter):
   - Increments whenever a rip process aborts due to fatal demuxing errors, severe disc damage, or storage failure.
3. **`dvd_ripper_active_jobs`** (Gauge):
   - Indicates how many FFmpeg processes or raw ISO dumping worker threads are actively running concurrently across all optical drives.
4. **`dvd_ripper_queued_jobs`** (Gauge):
   - Number of items waiting in the internal job queue (`JobQueueStore`).
5. **`dvd_ripper_progress_percent`** (Gauge):
   - Floating-point value from `0.0` to `100.0` reflecting the current progress percentage of the primary active rip task.

---

## 3. Prometheus Scrape Configuration

Add the following scrape job to your `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: "dvd_ripper"
    scrape_interval: 10s
    scrape_timeout: 5s
    static_configs:
      - targets: ["192.168.1.100:8080"]
        labels:
          instance: "nas-optical-appliance"
          environment: "production"
```

---

## 4. Grafana Dashboard Examples

### 4.1 Rip Progress Gauge Panel
- **Query (PromQL)**:
  ```promql
  dvd_ripper_progress_percent
  ```
- **Visualization**: Gauge (`0` to `100%`) with threshold coloring:
  - `0 - 50%`: Blue
  - `50 - 90%`: Yellow
  - `90 - 100%`: Green

### 4.2 Active Jobs & Concurrency
- **Query (PromQL)**:
  ```promql
  dvd_ripper_active_jobs
  ```
- **Visualization**: Stat / Bar Chart.

### 4.3 Total Discs Ripped Rate
- **Query (PromQL)**:
  ```promql
  increase(dvd_ripper_completed_rips_total[24h])
  ```
- **Visualization**: Time Series / Stat (Daily Rip Throughput).

---

## 5. Container & Kubernetes Health Probes

In Docker Compose or Kubernetes, use `/api/health` for automated container restarts and traffic readiness:

### 5.1 Docker Compose Healthcheck
```yaml
services:
  dvd-ripper:
    image: clockworkengineer/dvd-ripper:latest
    ports:
      - "8080:8080"
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/api/health"]
      interval: 30s
      timeout: 5s
      retries: 3
      start_period: 10s
```

### 5.2 Kubernetes Pod Spec
```yaml
livenessProbe:
  httpGet:
    path: /api/health
    port: 8080
  initialDelaySeconds: 15
  periodSeconds: 20
readinessProbe:
  httpGet:
    path: /api/health
    port: 8080
  initialDelaySeconds: 5
  periodSeconds: 10
```
