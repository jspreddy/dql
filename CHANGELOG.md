# Changelog

Generated from the first-parent history of `origin/v-rust` by `scripts/changelog.py`.
Do not edit by hand: changes are rewritten the next time it runs. Newest first.
A "Bump version" commit closes a section. Changes after that bump belong to the
next bump, or to Unreleased when no later bump exists.

## Unreleased (2026-11-13)

- Add dqlrs web --init-examples to the README banner (#199)

## dqlrs 0.7.9-dev0 (2026-11-12 to 2026-11-13)

- Color what the dqlrs command prints to the console (#196)
- Fix the vulnerabilities cargo audit and pip-audit report (#187)
- Add a filetype-report skill that renders a PNG of file types in git (#186)
- Bump certifi from 2026.7.22 to 2026.10.18 in /notebook (#184)
- Bump certifi from 2026.7.22 to 2026.10.18 in /notebook-tests (#181)
- Bump certifi from 2025.7.14 to 2026.10.18 in /py-impl (#179)
- github actions: Run just check and update the changelog (#175)

## dqlrs 0.7.8-dev0 (2026-11-12)

- Bump dqlrs to 0.7.8-dev0 and update the changelog (#173)
- Organize Rust docs, add a README banner, refresh screenshots (#170)

## dqlrs 0.7.7-dev0 (2026-11-11 to 2026-11-12)

- Make the listed-table search test create its own table (#172)
- Use local and temp table definitions in the linter and structure view (#155)

## dqlrs 0.7.6-dev0 (2026-11-10 to 2026-11-11)

- Bump Rust version: 0.7.5-dev0 → 0.7.6-dev0 (#153)
- Keep a folder open when a pending folder click fires (#152)
- Fix flaky file-tree drag and rename web tests (#147)
- Fix two racy web tests (#146)
- git-graph skill: always output a PNG (#145)
- Add branching rule: v-rust is the target branch, use stacked PRs (#143)
- notebook-tests: pass --allow-root to JupyterLab when running as root (a4b2ccc)
- Use per-file .md symlinks in .claude/rules for Cursor .mdc rules (652b4e8)
- Expose .cursor/rules to Claude Code via .claude/rules symlink (7d07a86)
- Expose .cursor/skills to Claude Code via .claude/skills symlink (2e90233)

## dqlrs 0.7.5-dev0 (2026-11-08)

- Replace browser confirm and prompt with in-app dialogs (#141)
- Add Sai Phaninder Reddy Jonnala's copyright to the MIT notice (#138)

## dqlrs 0.7.4-dev0 (2026-11-07 to 2026-11-08)

- Let the git graph take a commit count and a cutoff (#136)
- Write each git graph to a new SVG file (#135)

## dqlrs 0.7.3-dev0 (2026-11-07)

- Wait for the tree event in the events stream test (#134)
- Lint the open file on save, fetch, and open (#133)

## dqlrs 0.7.2-dev0 (2026-11-01 to 2026-11-07)

- Add LOCALTABLE and TEMPTABLE beside DynamoDB tables (#130)
- Use Playwright screenshots for web UI evidence (#131)
- Add Run file to the file context menu (#121)
- Keep the API and MCP token across restarts (#120)
- Add DynamoDB Local just aliases and dd-clean (#119)

## dqlrs 0.7.1-dev0 (2026-10-22 to 2026-10-30)

- Seal both disk caches with AES-256-GCM (#113)
- Make just check report-only and add just fix (#110)
- Keep the editor outline on the dragged lines (#109)
- Say when just verify succeeded, and describe the failure (#108)
- Load every table definition when the web server starts (#107)
- Add Ctrl+\ to run the selection (#106)
- rules (8c6660d)
- guard rails specificity (a4aa3e5)
- adding hooks and other guard rails for the examples directory (4db98d6)
- Refresh query files changed elsewhere and show connected clients (#103)
- Add a just-verify skill that reports headings and the exit code (#105)
- Add an MCP help tool for DQL syntax (#104)
- Refresh the day-binned work metrics page (#102)
- Show local and global indexes as tables on the describe card (#101)
- Fix the needless borrow flagged by rust-clippy (#100)
- Keep the row view button at the left edge while a table scrolls sideways (#99)
- Format the CREATE TABLE statement shown when describing a table (#98)
- Keep table column headers fixed while rows scroll (#97)
- Match LS, SHOW TABLES, and search against the table list (#96)
- skill + data (80f50b8)
- Add rewrite metrics to the Rust docs (#84)
- Drop the extra columns table ported from Python (#80)

## dqlrs 0.7.0-dev0 (2026-10-12 to 2026-10-22)

- Match analyze scans to scan cards and show real RCU (#95)
- Show the full structure sample and scroll sideways (#79)
- Scroll clear so the REPL input returns to the top (#62)
- Document installs from the v-rust branch (#56)
- just check (73be2bf)
- Reapply "Add a Stop control for a running web query." (8b13a78)

## dqlrs 0.6.5-dev1 (2026-10-10 to 2026-10-12)

- Fix WHERE contains() errors on reserved fields (#54)
- Soften key column borders to match the grid (#47)
- Cursor/grouped changes (#46)
- Fix duplicated editor imports that break the web UI (#40)
- Merge branch 'sai/test' into v-rust (a213428)

## dqlrs 0.6.5-dev0 (2026-08-20 to 2026-10-10)

- Add a floating play button for the current selection (#35)
- test(black-box-tests): fail the run when teardown DROP exits non-zero (9de655a)
- Sai/pexpect acceptance tests (#24)
- Sai/test case file renames (#18)
- Black-box SELECT with pk, sk, and extra filters (#16)
- Print hierarchical Setup/Input/Expect/Teardown in -v (#15)
- Black-box acceptance tests against DynamoDB Local (#14)
- Rust Implementation (#13)

## dql (Python) 0.6.4-dev12 (2026-07-02)

- Use `uv` (#4)

## dql (Python) 0.6.4-dev10 (2025-12-05)

- Bug: Index auto-select when not specified. (#5)

## dql (Python) 0.6.4-dev9 (2024-12-19 to 2025-08-01)

- 0.6.4-dev9: Rich format update + Bugfixes + CI (#3)
- formatting of changes.rst file (7790973)

## dql (Python) 0.6.4-dev8 (2024-10-17 to 2024-12-19)

- sai/dev (#2)
- github job changes (08fa8a1)
- readme updates for v-next (1863d23)

## dql (Python) 0.6.4-dev6 (2024-10-17)

- try again to fix packaging issue. (00a1be2)

## dql (Python) 0.6.4-dev5 (2024-10-17)

- Fixing packaging with pyproject.toml (5b8b042)
- readme (27cbdfe)
- Readme updates (65dfd68)

## dql (Python) 0.6.4-dev4 (2024-10-13)

- Fixing some cli prompt bugs. (0308870)

## dql (Python) 0.6.4-dev3 (2024-09-28)

- Most tests cases are passing with local dynamodb (131ecb2)

## dql (Python) 0.6.4-dev2 (2024-09-21 to 2024-09-28)

- removing setup.py and migrating to pyproject.toml completely (77fb8da)
- minor code formatting (77f5014)
- adding command whoami to print the session identity. (6a70572)

## dql (Python) 0.6.4-dev1 (2024-07-30 to 2024-09-21)

- BugFix: <LS> command breaks if using semicolon. - "ls tablename;" caused error on semicolon. Fixed it. (c1f38fa)
- trying to modernize the build / test tooling (056c365)
- Developer/maintainer guide link on main readme (1bc17ab)
- maintainer install notes (8a90990)
- lint fix (9418b4a)

## dql (Python) 0.6.4-dev0 (2024-05-24 to 2024-07-30)

- add ability to force "no index" when querying (9ee27ab)
- attempt to fix not selecting index (7d4e5d5)
- re-using progress bars (3c8a508)

## dql (Python) 0.6.3-dev1 (2022-11-01 to 2024-05-24)

- lint fixes for test cases (8ed003c)
- Adding progress bar to batching operations (3eccf1a)
- Fix errors with _attributes in query (f77f53f)
- fixing some lint issues (d1fc4a6)
- formatting using tox -e format (4159f43)
- Fixing some bugs, upgrading python, tox and other deps (5b26679)
- wip: ls command: rich rendering + updates (3d3ead6)
- wip: ls command: partial rich rendering (e6bc1cd)

## dql (Python) 0.6.3-dev0 (2021-12-23)

- wip: colorizing JsonFormat (7e28f19)
- doc: forgot to remove 'unreleased' from 0.6.2 (9666cfb)

## dql (Python) 0.6.2 (2021-12-07 to 2021-12-23)

- doc: update readme installation instructions (d8c2862)
- Adding clear command and excluding clear, exit from history (#43)
- chore: configure workflow to auto-publish on tag (continues #42) (4951ffe)
- doc: fix title underline in changelog (9383e32)
- doc: update the build badge in the readme (50574c2)

## dql (Python) 0.6.2-dev0 (2021-12-07)

- WIP: Fixing missing dependency (typing_extensions) + chores (#42)

## dql (Python) 0.6.1 (2021-01-24)

- Updating changelog (52290e1)
- Clean up history test (0656b25)
- Add .envrc for direnv (6e3fb27)

## dql (Python) 0.6.1-dev1 (2020-11-29 to 2021-01-24)

- Feature: Retain query history across sessions. (#41)
- Add a note about PartiQL in the README (d37f07f)

## dql (Python) 0.6.1-dev0 (2020-10-18 to 2020-11-23)

- Dynamo local path to be within project root + gitignore (#38)
- Fix #36: Constraint functions should accept quoted field names (4706ad7)
- Parser extracts values during parsing for conditional constraints (3c95123)
- Change the parsing logic for constraint expressions (e245229)
- Fix save and load file logic (03cfea1)
- Fix #37: Cannot count(*) on an index (4c6fb35)

## dql (Python) 0.6.0 (2020-09-09 to 2020-10-03)

- Updating changelog (01bfeb3)
- Update dynamo3 requirement (8d18520)
- Fix formatting (4f305c4)
- Updating changelog (dffa84e)
- Fix #34: ls on a table with global indexes causes AttributeError (ef604ea)
- Add snapshot test for ls format (9900a8a)
- Add a --json argument to output queries in line-by-line json (bfa6ea7)
- Add new formatter for JSON output (6a30999)
- Update changelog (33f9443)
- CHANGES.rst: new line (#33)
- Fix tests and lint (bfaef96)
- Fix #30: ls command accepts glob patterns (99d0216)
- Leave the stack trace out of the parse exception display (3cb64a1)
- Upgrade dynamo3 to pre-release version (aa86dd3)
- Fixed #31: On-Demand tables cause ZeroDivisionError with ls (#32)
- Enable mypy checking for untyped defs (550c86d)
- Add mypy as a test requirement (374e736)
- Make the mypy checking slightly stricter (5e73670)
- Replace old assertItemsEqual method with assertCountEqual (667067c)
- Add mypy checking in lint step (9e54053)
- Use requirements files to manage test/dev dependencies (f997bff)
- Better errors (#28)

## dql (Python) 0.5.29-dev0 (2020-08-20 to 2020-09-07)

- Development environment updates + CI updates to run with proper java version (#27)
- Typo (265a3ab)
- Remove python 2 code from the install.py file (192325b)
- Remove python 2-to-3 compatibility code (59fe0eb)
- Drop a python-dateutil version constraint that we no longer need (fba9d85)
- Drop official support for python 2 (7c90ef5)
- Reformat files with black and isort (7b1830d)
- Use isort with black, and provide a tox command for autoformatting (161db92)
- Remove run_autopep8.sh script (251f616)

## dql (Python) 0.5.28 (2019-11-20)

- Merge tag '0.5.28' (c8aaa84)

## dql (Python) 0.5.27 (2019-03-04 to 2019-10-26)

- Merge tag '0.5.27' (3d88d47)
- Format everything with black (e9d3aa2)
- Mention tox in docs on developing (1adf345)
- Fix linter errors (81d3f6f)
- Fix #20: Update docs for developing (b3e2611)

## dql (Python) 0.5.26 (2018-03-17)

- Merge tag '0.5.26' (fad509b)

## dql (Python) 0.5.25 (2017-12-10)

- Merge tag '0.5.25' (8d029a3)

## dql (Python) 0.5.24 (2017-09-12)

- Merge tag '0.5.24' (8ade0b3)

## dql (Python) 0.5.23 (2017-07-25)

- Merge tag '0.5.23' (c503ef3)

## dql (Python) 0.5.22 (2016-10-04)

- Merge tag '0.5.22' (ac71ccd)

## dql (Python) 0.5.21 (2016-09-06)

- Merge tag '0.5.21' (dac5e4b)

## dql (Python) 0.5.20 (2016-07-28)

- Merge tag '0.5.20' (2f178b9)

## dql (Python) 0.5.19 (2016-06-15)

- Merge tag '0.5.19' (06d2eb0)

## dql (Python) 0.5.18 (2016-05-18)

- Merge tag '0.5.18' (0c7ffbe)

## dql (Python) 0.5.17 (2016-05-03 to 2016-05-06)

- Merge tag '0.5.17' (bb930d1)
- Fix incorrect grammar for USING in doc examples (#5)

## dql (Python) 0.5.16 (2016-03-05)

- Merge tag '0.5.16' (187a9e6)

## dql (Python) 0.5.15 (2016-02-16)

- Merge tag '0.5.15' (b11ae98)

## dql (Python) 0.5.14 (2016-02-11)

- Merge tag '0.5.14' (2803a8f)

## dql (Python) 0.5.13 (2016-02-10)

- Merge tag '0.5.13' (837f27c)

## dql (Python) 0.5.12 (2016-02-09)

- Merge tag '0.5.12' (2cb3b60)

## dql (Python) 0.5.11 (2015-12-10)

- Merge tag '0.5.11' (6928521)

## dql (Python) 0.5.10 (2015-12-09)

- Merge tag '0.5.10' (0bf7845)

## dql (Python) 0.5.9 (2015-12-06)

- Merge tag '0.5.9' (6d150c3)

## dql (Python) 0.5.8 (2015-12-01)

- Merge tag '0.5.8' (daa8922)

## dql (Python) 0.5.7 (2015-12-01)

- Merge tag '0.5.7' (585a1fa)

## dql (Python) 0.5.6 (2015-12-01)

- Merge tag '0.5.6' (b977f68)

## dql (Python) 0.5.5 (2015-11-25)

- Merge tag '0.5.5' (335b1c7)

## dql (Python) 0.5.4 (2015-11-18)

- Merge tag '0.5.4' (406447c)

## dql (Python) 0.5.3 (2015-11-17)

- Merge tag '0.5.3' (72b181d)

## dql (Python) 0.5.2 (2015-11-13)

- Merge tag '0.5.2' (07e4e7a)

## dql (Python) 0.5.1 (2015-11-10)

- Merge tag '0.5.1' (3000ac5)

## dql (Python) 0.5.0 (2015-11-10)

- Merge tag '0.5.0' (a0df25b)

## dql (Python) 0.4.1 (2015-05-15)

- Merge tag '0.4.1' (b76e0a4)

## dql (Python) 0.4.0 (2014-10-13)

- Merge tag '0.4.0' (3cd143d)

## dql (Python) 0.3.2 (2014-08-26)

- Merge tag '0.3.2' (fa1da48)

## dql (Python) 0.3.1 (2014-08-20)

- Merge tag '0.3.1' (8a66b87)

## dql (Python) 0.3.0 (2014-06-27)

- Merge tag '0.3.0' (6f906b2)

## dql (Python) 0.2.1 (2013-12-05 to 2014-05-12)

- Merge tag '0.2.1' (8fdc8df)
- Updating documentation (6692a35)
- Use bumpversion to manage version number (a4904f0)
- Merge branch 'py3' (4ba9755)
- Documentation improvements (bdf8b72)
- Setting up for release (adaccee)
- Adding more tests (0b28787)
- Adding nose and mock to setup requirements (1d97bdd)
- Engine scope is persistent, not part of execute() (52d5bd1)
- Don't truncate fields in expanded format (d6d6c7f)
- DQL will update throughput in pieces if necessary (3a9531b)
- Smart formatting of query results (a13e978)
- Improving the output format (1e174b9)
- Increasing test coverage (2541e08)
- Fixing crash when describing a table being deleted (5059bef)
- Moving dev documentation from readme to docs (6e6c968)
- Better tab completion for table names (1a2c05a)
- Updating language to support different index projections (ac6a1df)
- Can autoload python commands from conf file (61a639f)
- Updating documentation (908b070)
- Allow more complex python expressions (210fc1f)
- Save format and display options to config file (6024665)
- Adding more output format options (c8e4611)
- Adding unittest2 as tox dep (aec78f0)
- Making sure we use unittest2 for tests (e43c911)
- Fixing tox deps for python 2.6 (dbfecd8)
- Adding support for atomic add/remove set operations (4c25764)
- Updating docs and moving tests outside package (e8502cd)
- Renaming version helper to avoid name collisions (185e630)
- Making delete queries more efficient (be4d95f)
- Fixing a formatting error in engine.py (4a739b0)
- Updating pylint config (40ae18a)
- New docs format (a95747a)
- Removing unused code (e98344e)
- Nose plugin silences dynamodb local output (cda0c67)
- Attempting to fix tests on python 2.6 (6d020d0)
- Attempting to fix build on python 2.6 (690ffa7)
- Updating nose plugin and docs for new DynamoDB Local (04dd78e)
- Correcting typo (efee2f6)
- Correcting typo (7ad9267)
- Adding support for global secondary indexes (545f624)
- Changing create table syntax (9b7f5a6)
- Can alter just read or just write throughput (1fffe97)
- Update boto requirements to fix DynamoDB api (51e9e67)
- Engine has a method for directly connecting to a region (31ac082)
- Default region can be set in environment variable (b09ff78)
- Further monkey patches around boto (6e9567e)
- Hacking a way around float rounding errors in boto (9151d75)
- Removing some unused functionality (8f0a0c1)
- Adding a way to run DQL inline with python (d324ec7)
- Engine has a scope and can do variable lookup (9ef7fb3)
- Adding autocomplete for ls (cce078d)
- Table description includes CloudWatch metrics (e572775)
- Client has option to run DQL from a file (5d5c941)
- Adding query help docs to the cli (5590f15)
- Updating docs for the scan 'IN' filter (48d1ae8)
- Changing setup.py version detection (1c5ee1d)
- Adding more of the WHERE and FILTER options (683f8c0)
- Better query string formatting (b14ffcd)
- The client now supports queries over multiple lines (296b688)
- Fixing coveralls link in README (f413711)
- Updating links in the README (02aea3b)
- Adding sphinx apidocs (d700006)
- Adding a bunch of documentation (8adeef3)
- Engine uses correct index name by default (b6dc5af)
- Select and count can force consistent reads (2c322ed)
- Select can reverse order of results (740474a)
- Engine can run multiple statements separated by ';' (e07a1ac)
- Found a bug in schema dumping (c8a2cdc)
- Adding a command to dump table schemas (cddde0a)
- Adding an ALTER statement to change throughput (3353afd)
- Create statement can specify throughput (245ff30)
- Removing dict comprehension for python 2.6 (941454a)
- Removing dict comprehension for python 2.6 (53816b9)
- Adding an update statement (b350bc3)
- Adding syntax for doing a batch get with SELECT (0c3e201)
- Better error reporting for repl (5ca7030)
- Adding a SCAN statement (5e6bc3b)
- Adding a new query for counting items (858f2a4)
- Select statement can fetch a subset of attributes (7d01b6f)
- Adding a 'limit' clause to the select query (21cf476)
- Updating the TODO (8a2f2fc)
- Fixing a bug in the query compiler (31f5fdb)
- Use unittest2 on python 2.6 (f20bbec)
- Require argparse on python 2.6 (d41cc1a)
- Putting tox requirement into travis.yml file (8d27b1c)
- Adding tox as a dev requirement (454de8b)
- Adding support for python 2.6 (a7704ac)
- Adding coveralls badge to README (d106e9d)
- Adding travis badge to README (2c2e5cf)
- And so begins the tweaking of shit to make the travis build pass (0624689)
- Allow passing in AWS credentials (3eaf6dc)
- First code commit (f466617)
- Initial commit (2c3bc57)
