#!/usr/bin/env python3
"""Build work-metrics JSON, Plotly graphs, and one HTML page.

Reads commit subjects, authors, and author dates, plus pull request titles
and merge times. Does not read diffs or file contents.
"""

from __future__ import annotations

import argparse
import html
import json
import math
import os
import subprocess
import tempfile
from collections import Counter, defaultdict
from datetime import datetime, timedelta, timezone
from pathlib import Path

import pandas as pd
import plotly
import plotly.graph_objects as go

MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]
# Hues sit far apart so stacked and neighboring bars stay readable on the cream page.
PALETTE = [
    "#E31A1C", "#0067A5", "#F38400", "#6A3D9A", "#2E7D32",
    "#E7298A", "#008888", "#882D17", "#9E9D24", "#8D6E63",
]
COMMIT_COLOR = "#E31A1C"
TOKEN_COLOR = "#1c1915"
HOUR_COLOR = "#0067A5"
TOKEN_BAR_COLOR = "#F38400"
PULL_COLOR = "#6A3D9A"
# 10 commits sit level with 40 million tokens on the timeline's right axis.
TOKENS_PER_COMMIT = 4.0
PHASES_PATH = Path(__file__).with_name("phases.json")

LAYOUT = dict(
    paper_bgcolor="#fffdf8",
    plot_bgcolor="#fffdf8",
    font=dict(family='"Avenir Next", "Segoe UI", sans-serif', color="#1c1915", size=13),
    margin=dict(l=58, r=68, t=48, b=52),
    legend=dict(orientation="h", yanchor="bottom", y=1.02, x=0, font=dict(size=12)),
    hovermode="closest",
)
AXIS = dict(gridcolor="#e3d9c8", zerolinecolor="#e3d9c8", linecolor="#d9cdb8", tickfont=dict(size=12))


def parse_dt(value: str) -> datetime:
    return datetime.fromisoformat(str(value).replace("Z", "+00:00")).astimezone(timezone.utc)


def iso_z(value: datetime) -> str:
    return value.astimezone(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def hour_key(value: datetime) -> str:
    return value.astimezone(timezone.utc).strftime("%Y-%m-%dT%H")


def bin_key(value: datetime, grain: str) -> str:
    value = value.astimezone(timezone.utc)
    if grain == "day":
        return value.strftime("%Y-%m-%d")
    if grain == "month":
        return value.strftime("%Y-%m")
    raise ValueError(grain)


def bin_stamp(key: str, grain: str) -> str:
    if grain == "day":
        return f"{key}T00:00:00Z"
    if grain == "month":
        return f"{key}-01T00:00:00Z"
    raise ValueError(grain)


def pretty_day(iso_date: str) -> str:
    _year, month, day = iso_date[:10].split("-")
    return f"{int(day)} {MONTHS[int(month) - 1]}"


def pretty_bin(key: str, grain: str) -> str:
    if grain == "month":
        year, month = key.split("-")
        return f"{MONTHS[int(month) - 1]} {year}"
    return pretty_day(key)


def day_counts(hour_keys: list[str]) -> str:
    counts = Counter(key[:10] for key in hour_keys)
    return "; ".join(f"{pretty_day(day)} ({counts[day]})" for day in sorted(counts))


def millions(value: float) -> float:
    return round(float(value) / 1e6, 1)


def headline_millions(value: float) -> str:
    return f"{int(value // 1_000_000):,}M"


def comma(value: float) -> str:
    return f"{int(round(value)):,}"


def load_phases() -> dict:
    data = json.loads(PHASES_PATH.read_text(encoding="utf-8"))
    seen: dict[str, str] = {}
    for area in data["areas"]:
        for key in area["hours"]:
            if key in seen:
                raise SystemExit(f"{key} is listed in both {seen[key]} and {area['id']}")
            seen[key] = area["id"]
    data["by_hour"] = seen
    return data


def git_env() -> dict[str, str]:
    env = os.environ.copy()
    env["TZ"] = "UTC"
    return env


def load_commits(repo: Path, base: str, head: str) -> list[dict]:
    raw = subprocess.check_output(
        ["git", "log", f"{base}..{head}", "--date=iso-strict", "--format=%ad%x09%an%x09%s"],
        cwd=repo,
        env=git_env(),
        text=True,
    )
    commits = []
    for line in raw.splitlines():
        if not line.strip():
            continue
        authored, author, subject = line.split("\t", 2)
        when = parse_dt(authored)
        commits.append(
            {
                "at": iso_z(when),
                "author": author,
                "subject": subject,
                "hour": hour_key(when),
                "day": when.strftime("%Y-%m-%d"),
                "_dt": when,
            }
        )
    commits.sort(key=lambda row: row["_dt"])
    return commits


def load_prs(repo: Path, base: str, head: str) -> list[dict]:
    raw = subprocess.check_output(
        [
            "gh",
            "pr",
            "list",
            "--state",
            "merged",
            "--limit",
            "200",
            "--json",
            "number,title,mergedAt,mergeCommit",
        ],
        cwd=repo,
        text=True,
    )
    listed = json.loads(raw)
    on_branch = set(
        subprocess.check_output(
            ["git", "log", f"{base}..{head}", "--format=%H"],
            cwd=repo,
            text=True,
        ).split()
    )
    kept = []
    for pr in listed:
        oid = (pr.get("mergeCommit") or {}).get("oid")
        merged_at = pr.get("mergedAt")
        if not oid or not merged_at or oid not in on_branch:
            continue
        when = parse_dt(merged_at)
        kept.append(
            {
                "number": pr["number"],
                "title": pr["title"],
                "merged_at": iso_z(when),
                "day": when.strftime("%Y-%m-%d"),
            }
        )
    kept.sort(key=lambda row: (row["merged_at"], row["number"]))
    return kept


def load_usage(csv_path: Path) -> pd.DataFrame:
    frame = pd.read_csv(csv_path)
    if "Date" not in frame.columns or "Total Tokens" not in frame.columns:
        raise SystemExit(f"{csv_path} is missing Date or Total Tokens")
    frame["when"] = frame["Date"].map(parse_dt)
    frame["hour"] = frame["when"].map(hour_key)
    frame["day"] = frame["when"].map(lambda value: value.strftime("%Y-%m-%d"))
    for column in (
        "Input (w/ Cache Write)",
        "Input (w/o Cache Write)",
        "Cache Read",
        "Output Tokens",
        "Total Tokens",
    ):
        if column not in frame.columns:
            frame[column] = 0
        frame[column] = pd.to_numeric(frame[column], errors="coerce").fillna(0)
    frame["dollars"] = pd.to_numeric(frame.get("Cost"), errors="coerce").fillna(0.0)
    frame["model"] = frame.get("Model", pd.Series(dtype=str)).fillna("").replace("", "(blank)")
    agent = frame.get("Cloud Agent ID", pd.Series(dtype=str)).fillna("").astype(str)
    frame["agent_id"] = agent.where(~agent.isin(["nan", "None"]), "")
    return frame


def load_repo_hours(repo: Path) -> set[str]:
    raw = subprocess.check_output(
        ["git", "log", "--all", "--date=iso-strict", "--format=%ad"],
        cwd=repo,
        env=git_env(),
        text=True,
    )
    hours = set()
    for line in raw.splitlines():
        if line.strip():
            hours.add(hour_key(parse_dt(line)))
    return hours


def starts_with_any(agent_id: str, prefixes: list[str]) -> bool:
    return any(str(agent_id).startswith(prefix) for prefix in prefixes)


def agent_label(agent_id: str, agents: list[list[str]]) -> str:
    if agent_id in ("", "nan"):
        return "No cloud agent id"
    for prefix, label in agents:
        if str(agent_id).startswith(prefix):
            return label
    return "Other agents"


def calendar_days(start: datetime, end: datetime) -> list[str]:
    days = []
    cursor = start.date()
    last = end.date()
    while cursor <= last:
        days.append(cursor.isoformat())
        cursor += timedelta(days=1)
    return days


def calendar_bins(start: datetime, end: datetime, grain: str) -> list[str]:
    if grain == "day":
        return calendar_days(start, end)
    bins = []
    cursor = start.astimezone(timezone.utc).replace(day=1, hour=0, minute=0, second=0, microsecond=0)
    last = end.astimezone(timezone.utc).replace(day=1, hour=0, minute=0, second=0, microsecond=0)
    while cursor <= last:
        bins.append(cursor.strftime("%Y-%m"))
        year = cursor.year + (1 if cursor.month == 12 else 0)
        month = 1 if cursor.month == 12 else cursor.month + 1
        cursor = cursor.replace(year=year, month=month)
    return bins


def hour_to_bin(hour: str, grain: str) -> str:
    return hour[:10] if grain == "day" else hour[:7]


BAR_WIDTH = {"day": 16 * 3600 * 1000, "month": 20 * 24 * 3600 * 1000}
HOVER_TIME = {"day": "%d %b", "month": "%b %Y"}


def apply_layout(fig: go.Figure, height: int) -> go.Figure:
    fig.update_layout(height=height, **LAYOUT)
    fig.update_xaxes(**AXIS)
    fig.update_yaxes(**AXIS)
    return fig


def fig_div(fig: go.Figure, div_id: str) -> str:
    return fig.to_html(
        full_html=False,
        include_plotlyjs=False,
        div_id=div_id,
        config={"displayModeBar": False, "responsive": True},
    )


def timeline_figure(days: list[str], hours: list[int], commits: list[int], token_m: list[float], grain: str) -> go.Figure:
    hover = HOVER_TIME[grain]
    fig = go.Figure()
    fig.add_bar(
        x=days,
        y=hours,
        name="Hours worked",
        marker_color=HOUR_COLOR,
        yaxis="y",
        width=BAR_WIDTH[grain],
        hovertemplate="%{x|" + hover + "}: %{y} hours worked<extra></extra>",
    )
    fig.add_scatter(
        x=days,
        y=token_m,
        name="Tokens (millions)",
        mode="lines",
        line=dict(color=TOKEN_COLOR, width=2),
        yaxis="y2",
        hovertemplate="%{x|" + hover + "}: %{y:.1f} million tokens<extra></extra>",
    )
    marked = [(day, count) for day, count in zip(days, commits) if count]
    if marked:
        mark_days, mark_counts = zip(*marked)
        largest = max(mark_counts)
        fig.add_scatter(
            x=list(mark_days),
            y=[count * TOKENS_PER_COMMIT for count in mark_counts],
            name="Commits",
            mode="markers",
            yaxis="y2",
            customdata=list(mark_counts),
            marker=dict(
                size=list(mark_counts),
                sizemode="area",
                sizeref=2.0 * largest / (34.0 ** 2),
                sizemin=5,
                color=COMMIT_COLOR,
                opacity=0.92,
                line=dict(color="#fffdf8", width=1.4),
            ),
            hovertemplate="%{x|" + hover + "}: %{customdata} commits<extra></extra>",
        )
    apply_layout(fig, 480)
    fig.update_layout(
        barmode="overlay",
        yaxis=dict(title="Hours worked", rangemode="tozero", **AXIS),
        yaxis2=dict(title="Tokens (millions)", overlaying="y", side="right", rangemode="tozero", **AXIS),
        xaxis=dict(title="", type="date", tickformat="%b" if grain == "day" else "%b %Y", dtick="M1", **AXIS),
    )
    return fig


def hours_figure(days: list[str], series: list[tuple[str, str, list[int]]], grain: str) -> go.Figure:
    hover = HOVER_TIME[grain]
    fig = go.Figure()
    for label, color, values in series:
        fig.add_bar(
            x=days,
            y=values,
            name=label,
            marker_color=color,
            width=BAR_WIDTH[grain],
            hovertemplate="%{x|" + hover + "}: %{y} " + label + "<extra></extra>",
        )
    apply_layout(fig, 440)
    fig.update_layout(
        barmode="stack",
        yaxis=dict(title="Hours worked", rangemode="tozero", **AXIS),
        xaxis=dict(type="date", tickformat="%b" if grain == "day" else "%b %Y", dtick="M1", **AXIS),
    )
    return fig


def token_bin_figure(rows: list[dict], grain: str, start: datetime, end: datetime) -> go.Figure:
    fig = go.Figure()
    fig.add_bar(
        x=[row["at"] for row in rows],
        y=[row["tokens_millions"] for row in rows],
        name="Tokens (millions)",
        marker_color=TOKEN_BAR_COLOR,
        hovertemplate="%{customdata}: %{y:.1f} million tokens<extra></extra>",
        customdata=[row["label"] for row in rows],
    )
    fig.update_traces(width=BAR_WIDTH[grain])
    apply_layout(fig, 420)
    fig.update_layout(
        yaxis=dict(title="Tokens (millions)", rangemode="tozero", **AXIS),
        xaxis=dict(
            type="date",
            tickformat="%b %Y" if grain == "month" else "%b",
            dtick="M1",
            range=[start.date().isoformat(), (end.date() + timedelta(days=1)).isoformat()],
            **AXIS,
        ),
        showlegend=False,
    )
    return fig


def pulls_figure(rows: list[dict]) -> go.Figure:
    fig = go.Figure()
    fig.add_bar(
        x=[row["label"] for row in rows],
        y=[row["count"] for row in rows],
        marker_color=PULL_COLOR,
        hovertemplate="%{x}: %{y} pull requests<extra></extra>",
    )
    apply_layout(fig, 360)
    fig.update_layout(
        yaxis=dict(title="Pull requests", rangemode="tozero", **AXIS),
        xaxis=dict(type="category", **AXIS),
        showlegend=False,
    )
    return fig


def horizontal_figure(rows: list[dict], value_key: str, suffix: str) -> go.Figure:
    fig = go.Figure()
    fig.add_bar(
        y=[row["label"] for row in rows],
        x=[row[value_key] for row in rows],
        orientation="h",
        marker_color=[row["color"] for row in rows],
        hovertemplate="%{y}: %{x:.1f}" + suffix + "<extra></extra>",
    )
    height = max(280, 36 * len(rows) + 90)
    apply_layout(fig, height)
    fig.update_layout(
        yaxis=dict(autorange="reversed", **AXIS),
        xaxis=dict(title=suffix.strip(), rangemode="tozero", **AXIS),
        showlegend=False,
        margin=dict(l=160, r=40, t=36, b=48),
    )
    return fig


def write_json(path: Path, payload: object) -> None:
    path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def build(args: argparse.Namespace) -> None:
    phases = load_phases()
    out = args.out or Path(tempfile.mkdtemp(prefix="work-metrics-"))
    out.mkdir(parents=True, exist_ok=True)
    commits = load_commits(args.repo, args.base, args.head)
    if not commits:
        raise SystemExit(f"No commits in {args.base}..{args.head}")
    usage = load_usage(args.csv)
    pulls = load_prs(args.repo, args.base, args.head)
    first = commits[0]["_dt"]
    last = commits[-1]["_dt"]
    span_hours = math.floor((last - first).total_seconds() / 3600)
    span_days = math.floor((last - first).total_seconds() / 86400)

    rewrite_hours = {row["hour"] for row in commits}
    repo_hours = load_repo_hours(args.repo)
    this_repo_prefixes = [prefix for prefix, _label in phases["agents"]]
    other_prefixes = [row["prefix"] for row in phases.get("other_repo_agents", [])]
    usage = usage.copy()
    usage["this_repo_agent"] = usage["agent_id"].map(lambda value: starts_with_any(value, this_repo_prefixes))
    usage["other_repo_agent"] = usage["agent_id"].map(lambda value: starts_with_any(value, other_prefixes))
    usage["repo_commit_hour"] = usage["hour"].isin(repo_hours)
    dropped_other = usage.loc[usage["other_repo_agent"]]
    dropped_no_commit = usage.loc[~usage["other_repo_agent"] & ~usage["this_repo_agent"] & ~usage["repo_commit_hour"]]
    dropped_other_tokens = int(dropped_other["Total Tokens"].sum())
    dropped_no_commit_tokens = int(dropped_no_commit["Total Tokens"].sum())
    usage = usage.loc[
        usage["this_repo_agent"] | (usage["repo_commit_hour"] & ~usage["other_repo_agent"])
    ].copy()
    phase_of = phases["by_hour"]
    outside_hours = sorted(hour for hour in rewrite_hours if hour not in phase_of)

    area_rows = []
    for area in phases["areas"]:
        keys = [key for key in area["hours"] if key in rewrite_hours]
        area_rows.append(
            {
                "id": area["id"],
                "label": area["label"],
                "color": area["color"],
                "summary": area["summary"],
                "hours": len(keys),
                "canon_hours": len(area["hours"]),
                "commits": sum(1 for row in commits if row["hour"] in set(area["hours"])),
                "dates": day_counts(keys),
            }
        )
    outside = phases["outside"]
    area_rows.append(
        {
            "id": outside["id"],
            "label": outside["label"],
            "color": outside["color"],
            "summary": outside["summary"],
            "hours": len(outside_hours),
            "canon_hours": len(outside_hours),
            "commits": sum(1 for row in commits if row["hour"] in set(outside_hours)),
            "dates": day_counts(outside_hours),
        }
    )

    usage["bin"] = usage["when"].map(lambda value: bin_key(value, args.bin))
    for row in commits:
        row["bin"] = bin_key(row["_dt"], args.bin)

    grouped: dict[str, list[dict]] = defaultdict(list)
    for row in commits:
        grouped[row["bin"]].append(row)
    commit_bins = []
    for key in sorted(grouped):
        rows = grouped[key]
        commit_bins.append(
            {
                "bin": key,
                "label": pretty_bin(key, args.bin),
                "at": bin_stamp(key, args.bin),
                "commits": [
                    {"at": row["at"], "author": row["author"], "subject": row["subject"]}
                    for row in rows
                ],
            }
        )

    token_groups = (
        usage.groupby("bin", sort=True)
        .agg(tokens=("Total Tokens", "sum"), dollars=("dollars", "sum"), events=("Total Tokens", "size"))
        .reset_index()
    )
    token_rows = []
    for record in token_groups.to_dict("records"):
        key = str(record["bin"])
        token_rows.append(
            {
                "bin": key,
                "label": pretty_bin(key, args.bin),
                "at": bin_stamp(key, args.bin),
                "tokens": int(record["tokens"]),
                "tokens_millions": millions(record["tokens"]),
                "dollars": round(float(record["dollars"]), 2),
                "events": int(record["events"]),
            }
        )

    if usage.empty:
        timeline_start, timeline_end = first, last
    else:
        timeline_start = min(first, usage["when"].min())
        timeline_end = max(last, usage["when"].max())
    bins = calendar_bins(timeline_start, timeline_end, args.bin)
    window_start = timeline_start.astimezone(timezone.utc).strftime("%Y-%m-%d")
    window_end = timeline_end.astimezone(timezone.utc).strftime("%Y-%m-%d")
    worked = {key for key in repo_hours if window_start <= key[:10] <= window_end}

    def phase_name(key: str) -> str:
        if key in rewrite_hours and key in phase_of:
            return phase_of[key]
        if key in rewrite_hours:
            return "outside"
        return "repo"

    commits_by_bin = Counter(bin_key(row["_dt"], args.bin) for row in commits)
    hours_by_bin: dict[str, Counter] = defaultdict(Counter)
    for key in sorted(worked):
        hours_by_bin[hour_to_bin(key, args.bin)][phase_name(key)] += 1
    tokens_by_bin = usage.groupby("bin")["Total Tokens"].sum() if not usage.empty else {}
    timeline_rows = []
    axis = []
    for key in bins:
        axis.append(bin_stamp(key, args.bin))
        timeline_rows.append(
            {
                "bin": key,
                "label": pretty_bin(key, args.bin),
                "commits": int(commits_by_bin.get(key, 0)),
                "hours_worked": int(sum(hours_by_bin[key].values())),
                "tokens": int(tokens_by_bin.get(key, 0)),
                "tokens_millions": millions(tokens_by_bin.get(key, 0)),
            }
        )

    hour_records = []
    for key in sorted(worked):
        hour_records.append(
            {
                "hour": key,
                "bin": hour_to_bin(key, args.bin),
                "commits": sum(1 for row in commits if row["hour"] == key),
                "tokens": int(usage.loc[usage["hour"] == key, "Total Tokens"].sum()),
                "phase": phase_name(key),
            }
        )

    for row in pulls:
        row["bin"] = hour_to_bin(row["day"] + "T00", args.bin)
    pull_counts = Counter(row["bin"] for row in pulls)
    pull_rows = [
        {"bin": key, "label": pretty_bin(key, args.bin), "count": pull_counts[key],
         "pulls": [row for row in pulls if row["bin"] == key]}
        for key in sorted(pull_counts)
    ]

    model_totals = usage.groupby("model")["Total Tokens"].sum().sort_values(ascending=False)
    model_rows = []
    top = list(model_totals.head(7).items())
    other = float(model_totals.iloc[7:].sum()) if len(model_totals) > 7 else 0.0
    for index, (name, value) in enumerate(top):
        model_rows.append(
            {"label": str(name), "tokens": int(value), "tokens_millions": millions(value), "color": PALETTE[index]}
        )
    if other:
        model_rows.append(
            {"label": "Other", "tokens": int(other), "tokens_millions": millions(other), "color": PALETTE[7]}
        )

    usage["agent"] = usage["agent_id"].map(lambda value: agent_label(value, phases["agents"]))
    agent_totals = usage.groupby("agent")["Total Tokens"].sum().sort_values(ascending=False)
    agent_rows = []
    for index, (name, value) in enumerate(agent_totals.items()):
        if value <= 0:
            continue
        agent_rows.append(
            {
                "label": str(name),
                "tokens": int(value),
                "tokens_millions": millions(value),
                "color": PALETTE[index % len(PALETTE)],
            }
        )

    total_tokens = int(usage["Total Tokens"].sum())
    dollars = round(float(usage["dollars"].sum()), 2)
    stack_order = [area["id"] for area in phases["areas"]] + ["outside", "repo"]
    stack_meta = {area["id"]: (area["label"], area["color"]) for area in phases["areas"]}
    stack_meta["outside"] = (outside["label"], outside["color"])
    stack_meta["repo"] = (phases["repo_commits"]["label"], phases["repo_commits"]["color"])
    stack_series = []
    for phase_id in stack_order:
        label, color = stack_meta[phase_id]
        stack_series.append((label, color, [int(hours_by_bin[key].get(phase_id, 0)) for key in bins]))

    summary = {
        "bin": args.bin,
        "base": args.base,
        "head": args.head,
        "commits": len(commits),
        "first_commit": iso_z(first),
        "last_commit": iso_z(last),
        "span_days": span_days,
        "span_hours": span_hours,
        "hours_worked": len(worked),
        "rewrite_hours": sum(1 for key in worked if key in rewrite_hours),
        "repo_only_hours": sum(1 for key in worked if key not in rewrite_hours),
        "pull_requests": len(pulls),
        "tokens": total_tokens,
        "dollars": dollars,
        "dropped_other_repo_tokens": dropped_other_tokens,
        "dropped_no_commit_tokens": dropped_no_commit_tokens,
        "timeline_start": bins[0],
        "timeline_end": bins[-1],
        "areas": area_rows,
    }

    public_commits = [{key: value for key, value in row.items() if key != "_dt"} for row in commits]
    write_json(out / "commits-by-bin.json", {"bin": args.bin, "commits": len(commits), "bins": commit_bins})
    write_json(out / "commits.json", public_commits)
    write_json(out / "timeline.json", {"bin": args.bin, "start": bins[0], "end": bins[-1], "bins": timeline_rows})
    write_json(out / "tokens-by-bin.json", {"bin": args.bin, "tokens": total_tokens, "dollars": dollars, "bins": token_rows})
    write_json(out / "pulls-by-day.json", {"pull_requests": len(pulls), "days": pull_rows})
    write_json(out / "tokens-by-agent.json", {"agents": agent_rows})
    write_json(out / "tokens-by-model.json", {"models": model_rows})
    write_json(
        out / "hours-worked.json",
        {
            "definition": "A UTC hour is worked when it has a commit on any ref in this repo. The report sums those hours into the chosen bin.",
            "hours_worked": len(worked),
            "rewrite_hours": sum(1 for key in worked if key in rewrite_hours),
            "repo_only_hours": sum(1 for key in worked if key not in rewrite_hours),
            "hours": hour_records,
        },
    )
    write_json(out / "summary.json", summary)

    figures = {
        "timeline": timeline_figure(
            axis,
            [row["hours_worked"] for row in timeline_rows],
            [row["commits"] for row in timeline_rows],
            [row["tokens_millions"] for row in timeline_rows],
            args.bin,
        ),
        "hours": hours_figure(axis, stack_series, args.bin),
        "tokens": token_bin_figure(token_rows, args.bin, timeline_start, timeline_end),
        "pulls": pulls_figure(pull_rows),
        "models": horizontal_figure(model_rows, "tokens_millions", " million tokens"),
        "agents": horizontal_figure(agent_rows, "tokens_millions", " million tokens"),
    }
    for name, fig in figures.items():
        (out / f"{name}.html").write_text(fig_div(fig, name), encoding="utf-8")

    page = render_html(summary, area_rows, figures, args)
    html_path = args.html or (out / "work-metrics.html")
    html_path.parent.mkdir(parents=True, exist_ok=True)
    html_path.write_text(page, encoding="utf-8")
    if html_path.resolve() != (out / "work-metrics.html").resolve():
        (out / "work-metrics.html").write_text(page, encoding="utf-8")

    print(f"out: {out}")
    print(f"html: {html_path}")
    print(
        "summary:",
        json.dumps(
            {key: summary[key] for key in (
                "bin", "commits", "span_days", "span_hours", "hours_worked",
                "rewrite_hours", "repo_only_hours", "pull_requests",
                "tokens", "dollars", "dropped_other_repo_tokens", "dropped_no_commit_tokens",
            )}
        ),
    )
    print("areas:", [(row["label"], row["hours"], row["commits"]) for row in area_rows])


def render_html(summary: dict, areas: list[dict], figures: dict[str, go.Figure], args: argparse.Namespace) -> str:
    bin_label = {"day": "day", "month": "month"}[summary["bin"]]
    first = pretty_stamp(summary["first_commit"])
    last = pretty_stamp(summary["last_commit"])
    dollars = f"${summary['dollars']:.2f}"
    area_table = [
        "<table>",
        "<thead><tr><th>Area</th><th class=\"num\">Hours</th><th class=\"num\">Commits</th><th>Hours per day</th></tr></thead>",
        "<tbody>",
    ]
    for area in areas:
        area_table.append(
            "<tr>"
            f"<td>{html.escape(area['label'])}</td>"
            f"<td class=\"num\">{area['hours']}</td>"
            f"<td class=\"num\">{area['commits']}</td>"
            f"<td>{html.escape(area['dates'])}</td>"
            "</tr>"
        )
    area_table.append("</tbody></table>")
    stats = [
        ("Hours worked", comma(summary["hours_worked"])),
        ("Hours since start", comma(summary["span_hours"])),
        ("Commits", comma(summary["commits"])),
        ("Pull requests merged", comma(summary["pull_requests"])),
        ("Tokens", headline_millions(summary["tokens"])),
    ]
    stat_html = "".join(
        f'<div class="stat"><b>{html.escape(value)}</b><span>{html.escape(label)}</span></div>'
        for label, value in stats
    )
    plotly_js = plotly.offline.get_plotlyjs().replace("</", "<\\/")
    grain_note = (
        f"Each point is one {bin_label}. "
        "Hours worked are UTC hours that contain a commit, summed into that "
        f"{bin_label}."
    )
    return f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Work metrics, {html.escape(args.base)} to {html.escape(args.head)}</title>
  <script>{plotly_js}</script>
  <style>
    :root {{
      --bg: #f3efe6;
      --paper: #fffdf8;
      --ink: #1c1915;
      --muted: #5e564c;
      --line: #e3d9c8;
      --accent: #E31A1C;
      --code: #f6f1e8;
    }}
    * {{ box-sizing: border-box; }}
    html {{ scroll-behavior: smooth; }}
    body {{
      margin: 0;
      background: var(--bg);
      color: var(--ink);
      font: 17px/1.55 "Iowan Old Style", Palatino, "Palatino Linotype", Georgia, serif;
    }}
    a {{ color: var(--accent); }}
    code {{
      font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace;
      font-size: 0.86em;
      background: var(--code);
      padding: 0.1em 0.35em;
      border-radius: 3px;
    }}
    .layout {{
      display: grid;
      grid-template-columns: 16rem minmax(0, 52rem);
      gap: 2.5rem;
      max-width: 76rem;
      margin: 0 auto;
      padding: 2rem 1.5rem 4rem;
    }}
    nav {{
      position: sticky;
      top: 1.25rem;
      align-self: start;
      font-family: "Avenir Next", "Segoe UI", sans-serif;
      font-size: 0.84rem;
      line-height: 1.35;
    }}
    nav p {{
      margin: 0 0 0.6rem;
      color: var(--muted);
      letter-spacing: 0.04em;
      text-transform: uppercase;
      font-size: 0.72rem;
    }}
    nav ol {{ margin: 0; padding: 0; list-style: none; }}
    nav li {{ margin: 0.35rem 0; }}
    nav a {{ color: var(--ink); text-decoration: none; }}
    nav a:hover {{ color: var(--accent); }}
    header h1 {{
      font-weight: 600;
      font-size: 2.15rem;
      line-height: 1.15;
      margin: 0 0 0.6rem;
    }}
    .kicker {{
      font-family: "Avenir Next", "Segoe UI", sans-serif;
      font-size: 0.75rem;
      letter-spacing: 0.08em;
      text-transform: uppercase;
      color: var(--accent);
      margin: 0 0 0.4rem;
    }}
    h2 {{
      font-size: 1.55rem;
      font-weight: 600;
      margin: 2.4rem 0 0.6rem;
      padding-top: 0.4rem;
    }}
    p {{ margin: 0.7rem 0; }}
    .lead {{ font-size: 1.12rem; margin: 0 0 1rem; }}
    .stats {{
      display: grid;
      grid-template-columns: repeat(5, minmax(0, 1fr));
      gap: 0.7rem;
      margin: 1rem 0 0.4rem;
    }}
    .stat {{
      background: var(--paper);
      border: 1px solid var(--line);
      border-top: 3px solid var(--accent);
      padding: 0.75rem 0.7rem 0.65rem;
    }}
    .stat:nth-child(2) {{ border-top-color: #0067A5; }}
    .stat:nth-child(3) {{ border-top-color: #F38400; }}
    .stat:nth-child(4) {{ border-top-color: #6A3D9A; }}
    .stat:nth-child(5) {{ border-top-color: #2E7D32; }}
    .stat b {{
      display: block;
      font-family: "Avenir Next", "Segoe UI", sans-serif;
      font-size: 1.55rem;
      font-weight: 650;
      line-height: 1.1;
    }}
    .stat span {{
      display: block;
      margin-top: 0.25rem;
      color: var(--muted);
      font-family: "Avenir Next", "Segoe UI", sans-serif;
      font-size: 0.78rem;
      line-height: 1.3;
    }}
    .figure {{
      background: var(--paper);
      border: 1px solid var(--line);
      border-radius: 8px;
      padding: 0.25rem 0.35rem 0.15rem;
      margin: 0.7rem 0 0.2rem;
    }}
    .pair {{
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 1.25rem;
      align-items: start;
    }}
    table {{
      width: 100%;
      border-collapse: collapse;
      background: var(--paper);
      font-size: 0.95rem;
      margin: 0.8rem 0 1rem;
    }}
    th, td {{
      text-align: left;
      vertical-align: top;
      border-bottom: 1px solid var(--line);
      padding: 0.45rem 0.6rem;
    }}
    th {{
      font-family: "Avenir Next", "Segoe UI", sans-serif;
      font-size: 0.78rem;
      letter-spacing: 0.03em;
      text-transform: uppercase;
      color: var(--muted);
      font-weight: 600;
    }}
    td.num, th.num {{ text-align: right; font-variant-numeric: tabular-nums; }}
    .note {{
      background: var(--paper);
      border: 1px solid var(--line);
      border-left: 3px solid var(--accent);
      padding: 0.8rem 1rem;
      margin: 1rem 0;
    }}
    .caption {{ color: var(--muted); font-size: 0.92rem; margin: 0.35rem 0 0.2rem; }}
    footer {{ margin-top: 2.5rem; color: var(--muted); font-size: 0.92rem; }}
    @media (max-width: 860px) {{
      .layout {{ grid-template-columns: 1fr; padding: 1.2rem 1rem 3rem; }}
      nav {{ position: static; }}
      .stats, .pair {{ grid-template-columns: 1fr 1fr; }}
    }}
  </style>
</head>
<body>
  <div class="layout">
    <nav>
      <p>On this page</p>
      <ol>
        <li><a href="#summary">Summary</a></li>
        <li><a href="#areas">Areas of work</a></li>
        <li><a href="#timeline">Timeline</a></li>
        <li><a href="#hours-worked">Hours worked</a></li>
        <li><a href="#token-spend">Token spend</a></li>
        <li><a href="#pull-requests">Pull requests</a></li>
        <li><a href="#models">Tokens by model</a></li>
        <li><a href="#agents">Tokens by cloud agent</a></li>
      </ol>
    </nav>
    <main>
      <header>
        <p class="kicker">{html.escape(bin_label)} bin</p>
        <h1>Work metrics</h1>
      </header>
      <h2 id="summary">Summary</h2>
      <p class="lead">{comma(summary["commits"])} commits landed between {first} and {last}, a span of {comma(summary["span_days"])} days ({comma(summary["span_hours"])} hours). {comma(summary["hours_worked"])} hours were worked. {comma(summary["pull_requests"])} pull requests were merged. Token spend kept for this repo is {headline_millions(summary["tokens"])} tokens and {dollars}.</p>
      <div class="stats">{stat_html}</div>
      <p class="caption">Commits are <code>{html.escape(args.base)}..{html.escape(args.head)}</code>, dated from the author timestamp in UTC. Hours since start is that span, floored. An hour is worked when it has a commit on any ref in this repo. {comma(summary["rewrite_hours"])} of those hours are rewrite commits. {comma(summary["repo_only_hours"])} more have some other commit in this repo. The page sums those hours into each {html.escape(bin_label)}.</p>
      <p class="note">A usage row stays when its cloud agent is on this repo, even if that hour has no commit. A row with no agent id stays only when that UTC hour has a commit here. <code>bc-ca207e1b</code> is another repo, so its {headline_millions(summary["dropped_other_repo_tokens"])} tokens are left out. {headline_millions(summary["dropped_no_commit_tokens"])} more tokens had no agent id and no commit in this repo that hour. {dollars} is the numeric Cost on the rows that stay. Included and Free rows have no dollar amount. Token counts are not turned into prices. The timeline draws every {html.escape(bin_label)} from {html.escape(pretty_bin(summary["timeline_start"], summary["bin"]))} through {html.escape(pretty_bin(summary["timeline_end"], summary["bin"]))}.</p>

      <h2 id="areas">Areas of work</h2>
      <p>Each rewrite commit hour is counted once, in the area canonized for that hour. A calendar day can hold more than one area. Rust impl stops on 20 Aug. October query, <code>ls</code>, SAVE, and REPL hours are Rust improvements.</p>
      {"".join(area_table)}
      <p class="caption">The number after each date is commit hours that day. {html.escape(areas[-1]["summary"])}</p>

      <h2 id="timeline">Timeline</h2>
      <p>{grain_note} Bars are hours worked. The line is tokens, in millions. Each circle is one day's commits, and a larger circle is more commits.</p>
      <div class="figure">{fig_div(figures["timeline"], "timeline")}</div>
      <p class="caption">Every {html.escape(bin_label)} from {html.escape(pretty_bin(summary["timeline_start"], summary["bin"]))} through {html.escape(pretty_bin(summary["timeline_end"], summary["bin"]))} stays on the axis, including the empty stretches. Commit circles sit on the token axis at 4 million tokens per commit, so 10 commits are level with 40 million. Hover a bar, a point, or a circle for that {html.escape(bin_label)}.</p>

      <h2 id="hours-worked">Hours worked</h2>
      <p>An hour counts when it has a commit in this repo. The stack is the canonized area of a rewrite commit hour, summed into each {html.escape(bin_label)}.</p>
      <div class="figure">{fig_div(figures["hours"], "hours")}</div>
      <p class="caption">Other commits in this repo are hours with a commit on some ref here, outside the rewrite range. Empty {html.escape(bin_label)}s stay on the axis.</p>

      <h2 id="token-spend">Token spend by {html.escape(bin_label)}</h2>
      <div class="figure">{fig_div(figures["tokens"], "tokens")}</div>
      <p class="caption">Millions of tokens kept for this repo, one bar per {html.escape(bin_label)}. The axis keeps the full span.</p>

      <h2 id="pull-requests">Pull requests merged, by day</h2>
      <div class="figure">{fig_div(figures["pulls"], "pulls")}</div>
      <p class="caption">{comma(summary["pull_requests"])} merged pull requests whose merge commit is in the rewrite range.</p>

      <h2 id="models">Tokens by model</h2>
      <div class="figure">{fig_div(figures["models"], "models")}</div>
      <p class="caption">Millions of tokens. Other is every model after the first seven.</p>

      <h2 id="agents">Tokens by cloud agent</h2>
      <div class="figure">{fig_div(figures["agents"], "agents")}</div>
      <p class="caption">Named agents are the cloud agents on this repo. Their usage stays even in an hour with no commit. Rows with no id stay together, and only in hours that have a commit here.</p>
      <footer>
        <p>Day and hour pages from the earlier pass: <a href="rewrite-metrics.html">rewrite-metrics.html</a>, <a href="rewrite-metrics-hours.html">rewrite-metrics-hours.html</a>. Back to the <a href="README.md">Rust user guide</a>.</p>
      </footer>
    </main>
  </div>
</body>
</html>
"""


def pretty_stamp(iso: str) -> str:
    when = parse_dt(iso)
    hour = when.strftime("%H:%M")
    return f"{when.day} {MONTHS[when.month - 1]} {when.year} {hour} UTC"


def main() -> None:
    parser = argparse.ArgumentParser(description="Build work metrics from git history and a Cursor usage CSV.")
    parser.add_argument("--bin", default="day", choices=["day", "month"])
    parser.add_argument("--csv", required=True, type=Path, help="Cursor token-spend usage export")
    parser.add_argument("--out", type=Path, help="Temp directory for JSON and graph fragments")
    parser.add_argument("--html", type=Path, help="Combined HTML page. Defaults to OUT/work-metrics.html")
    parser.add_argument("--repo", type=Path, default=Path("."))
    parser.add_argument("--base", default="origin/v-next")
    parser.add_argument("--head", default="origin/v-rust")
    build(parser.parse_args())


if __name__ == "__main__":
    main()
