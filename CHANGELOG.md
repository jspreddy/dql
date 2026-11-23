# Changelog

Generated from the first-parent history of `origin/v-rust` by `scripts/changelog.py`.
Do not edit by hand: changes are rewritten the next time it runs. Newest first.
A "Bump version" commit closes a section. Changes after that bump belong to the
next bump, or to Unreleased when no later bump exists. Each line is a commit; the
latest commit of each pull request, stacked ones included, names that pull request.

## Unreleased (2026-11-23)

Contributors: [Sai Jonnala](https://github.com/jspreddy)\
Agents: Claude Opus 5.5, Cursor Agent

- [a5a3962](https://github.com/jspreddy/dql/commit/a5a3962aaf492da770d1039fa330291d175847e3) feat(web): show the capacity pill on write result lines ([#300](https://github.com/jspreddy/dql/pull/300))
- [65c52e4](https://github.com/jspreddy/dql/commit/65c52e4c644e2e394a4451dc65637fb49c07804b) fix(engine): count the scan behind UPDATE and DELETE WHERE as read units ([#297](https://github.com/jspreddy/dql/pull/297))
- [53cdb33](https://github.com/jspreddy/dql/commit/53cdb3389b57d1753822324a11c468b41a669914) fix(engine): count write calls' consumed capacity as write units ([#296](https://github.com/jspreddy/dql/pull/296))
- [5e8e924](https://github.com/jspreddy/dql/commit/5e8e924fd24c48ccc664cf769652f3cf0aca25a5) fix(web): open Run folder while a view-only file is open ([#295](https://github.com/jspreddy/dql/pull/295))
- [e4c45d6](https://github.com/jspreddy/dql/commit/e4c45d673ca93b6f996478b8476b97bc6f8bab5e) chore(playwright): refine screenshot cleanup logic in global setup ([#289](https://github.com/jspreddy/dql/pull/289))
- [cb79f31](https://github.com/jspreddy/dql/commit/cb79f31f3a1ccc528bc5c2916ce912a043e2f1b1) chore(playwright): enhance screenshot handling in tests
- [831b015](https://github.com/jspreddy/dql/commit/831b0152386cc4fdc2d9a59f2ae0a400b8e09533) chore(playwright):  exclude `.dqlrs-cache` from test workspace copies
- [fde3bff](https://github.com/jspreddy/dql/commit/fde3bff55874c2aebe778fec518b5dfb514a370c) fix(web): keep the Cosmos screenshot tests from stalling on WSL
- [9088799](https://github.com/jspreddy/dql/commit/9088799c111544fc0f69d2faee57aae6ac1c822b) feat(lint): check joins and WITH queries, and point at the word
- [3aed061](https://github.com/jspreddy/dql/commit/3aed06162aaf111bf82b0d08ecbc73d0a4dfb35e) chore(just): add a flake recipe that repeats a test recipe on fewer cores ([#292](https://github.com/jspreddy/dql/pull/292))
- [61e1055](https://github.com/jspreddy/dql/commit/61e10556a21e1f15c845f13cae9df2c8b6d3f7da) docs(examples): describe the library join in the README diagram ([#288](https://github.com/jspreddy/dql/pull/288))
- [92bed1b](https://github.com/jspreddy/dql/commit/92bed1b4720a6a2156dc6a42cbfb2d772028a70a) docs(examples): join a library hold to its shelf and desk
- [b361436](https://github.com/jspreddy/dql/commit/b36143612dd599a6d7ae6fe83a034df2006ca981) docs(examples): use everyday stories in the join scripts
- [30b4434](https://github.com/jspreddy/dql/commit/30b443465afcbd2367802a6866ed24d3f5585ec9) test(mcp): expect request_access in the smoke tool list ([#287](https://github.com/jspreddy/dql/pull/287))
- [2d9b6a0](https://github.com/jspreddy/dql/commit/2d9b6a0564053f470dd7646f4cf7a1f01cdee12c) fix(web): snap the MCP pulse and wait longer between pings
- [d9daf19](https://github.com/jspreddy/dql/commit/d9daf192d75254ba94dd42bcf6a90d5d70c04ef5) fix(web): pulse the MCP icon while a request waits
- [64a54dc](https://github.com/jspreddy/dql/commit/64a54dc7157f4bbfacca81d3c17fcf6b90b7f715) fix(web): ping the MCP icon instead of opening the panel
- [c21c2dd](https://github.com/jspreddy/dql/commit/c21c2dd29942987ec48ce962fda4f61c5dc776db) fix(web): match an 11-digit E.164 phone ([#286](https://github.com/jspreddy/dql/pull/286))
- [3cd50ca](https://github.com/jspreddy/dql/commit/3cd50cae74535db315359c85284329eacf846db0) fix(web): accept only US phone groupings
- [2c5c1db](https://github.com/jspreddy/dql/commit/2c5c1db4e7482ce8720fa408b46040d23a4b7494) fix(web): share query colors and label the grant panel
- [e86b585](https://github.com/jspreddy/dql/commit/e86b5856d18026137ab957e9eac5f63a406abe6a) fix(web): match a phone only as its own span
- [b6372bf](https://github.com/jspreddy/dql/commit/b6372bf6ac3893a2ccbbed5120be47a46606bdbb) fix(web): show access rows in the result table
- [917ce9d](https://github.com/jspreddy/dql/commit/917ce9d03eda8a936db5ca339527343e5ca2226e) fix(web): highlight the requested query and use the alert card
- [36eadbf](https://github.com/jspreddy/dql/commit/36eadbf1e921e8ac8f5d2b07eef31fb65434bbe7) fix(web): title the grant panel Access requested via MCP
- [ab1b323](https://github.com/jspreddy/dql/commit/ab1b323d22d8be534b11ffa0a7e75af1762c3351) fix(web): keep the grant panel in step with drops and later events
- [c1b071f](https://github.com/jspreddy/dql/commit/c1b071f149e67d7cfc219e32eb27f8b21d3db10a) feat(web): let an agent request a query and the user grant the result
- [35bfe96](https://github.com/jspreddy/dql/commit/35bfe9686ccc99e854dbd3cef8b6eb0054738846) docs: align the MCP grant plans with the server code ([#283](https://github.com/jspreddy/dql/pull/283))
- [9bedd7c](https://github.com/jspreddy/dql/commit/9bedd7c5c0f3eccb4722d02dffb28b78eff8dc60) docs(plans): add the MCP grant workflow above the three plans ([#281](https://github.com/jspreddy/dql/pull/281))
- [75f3d41](https://github.com/jspreddy/dql/commit/75f3d416401df7ac338ce3dfbf7b12983359309f) docs(plans): join the MCP grant plans into one flow
- [ae729d8](https://github.com/jspreddy/dql/commit/ae729d80a695316fcc727b8ccd5eaac291da970e) docs(plans): plan sensitive-data recommendations on grant requests
- [7d1ee13](https://github.com/jspreddy/dql/commit/7d1ee1330524c65122564700f98cb19f0dbb686e) docs(plans): plan the MCP grant panel on the header chip
- [169e597](https://github.com/jspreddy/dql/commit/169e5970d0dee2788b363e50f7a93a2edcd709b4) docs(plans): plan MCP grant and revoke events
- [4946059](https://github.com/jspreddy/dql/commit/4946059609ce5f10206db28d81c62e759f00da93) fix(lint): report the engine's errors for a WITH name that clashes ([#285](https://github.com/jspreddy/dql/pull/285))
- [b69c867](https://github.com/jspreddy/dql/commit/b69c867acf28ba379c1e107d95f8ae827d36a5f7) Move the PR viz skill folder to pr-viz ([#280](https://github.com/jspreddy/dql/pull/280))
- [ff39880](https://github.com/jspreddy/dql/commit/ff39880874547a68fe7d2e0c83b9e861add4f924) Rename the PR viz skill folder to visualize-pr
- [d15456d](https://github.com/jspreddy/dql/commit/d15456da8bbb4e7be93c973639d54866cf9db331) feat(skills): name the pull request file list PR viz
- [694eeb8](https://github.com/jspreddy/dql/commit/694eeb80d138c7f6a29c311147e99ffaa2533c30) feat(skills): scaffold a grouped pull request file list
- [9ca9138](https://github.com/jspreddy/dql/commit/9ca9138b55323213d009852be8648dc8356591af) fix(lint): treat a WITH name as separate from a dropped temp table ([#276](https://github.com/jspreddy/dql/pull/276))
- [19f5514](https://github.com/jspreddy/dql/commit/19f55143deea34a19e9cee4bd63a78510adae141) test(web): count WITH history cards on the History pane
- [a53ca8f](https://github.com/jspreddy/dql/commit/a53ca8f632dd52bf47db58248f383d4e638635bc) fix(engine): treat a short name as free before DescribeTable
- [44c0d5e](https://github.com/jspreddy/dql/commit/44c0d5e3f58b15461df39125793d3d688137b88c) test(web): reuse the same WITH names in Playwright
- [96776b2](https://github.com/jspreddy/dql/commit/96776b2d15d0f2842a2a90449bcb422b2ccc8b3f) test(engine): reuse the same WITH names in two queries
- [59599c9](https://github.com/jspreddy/dql/commit/59599c974c3c33b4137c83dbb7189441b6ed411a) fix(engine): keep WITH rows out of the temp catalog
- [98a1d66](https://github.com/jspreddy/dql/commit/98a1d66cab53c3042ed10c91a53191705c18686a) fix(engine): let a short WITH name stay in memory
- [b924cf7](https://github.com/jspreddy/dql/commit/b924cf717ae9d1dcc2cfd4316ce8872bb0fe3a5e) docs(examples): add joins scripts for the in-memory equijoin cases
- [5b2186f](https://github.com/jspreddy/dql/commit/5b2186f7d5ffd96c43c1670d9057a9bdfaebf5cb) fix(skills): mute commit subjects on the dark git graph ([#277](https://github.com/jspreddy/dql/pull/277))
- [74cd3a2](https://github.com/jspreddy/dql/commit/74cd3a25ea96783ca076d23ccdeaf15099c4e6bc) feat(skills): render the git graph on a dark ground
- [f522ceb](https://github.com/jspreddy/dql/commit/f522ceb340cade67a8072c9102f0129c03aab58f) test(mcp): expect granted-table tools in the stdio smoke check ([#271](https://github.com/jspreddy/dql/pull/271))
- [0dd2eb4](https://github.com/jspreddy/dql/commit/0dd2eb419e7de5bae1fdb70a8cd50b272279e84a) docs: describe MCP temp table grants and archive the plan
- [9196540](https://github.com/jspreddy/dql/commit/91965402ec7c9ea46fb0c0cb2b729fb8c5e792f7) feat(web): mark granted temp tables and record MCP reads
- [966ef93](https://github.com/jspreddy/dql/commit/966ef9361ee47d96e72c89e9e9ad0d2fffeb2813) feat(cli): list and read granted temp tables over MCP
- [c302997](https://github.com/jspreddy/dql/commit/c302997c66f962a4d86e788ed9bea3412394962f) feat(engine): grant MCP read on a temp table generation
- [3fd8d26](https://github.com/jspreddy/dql/commit/3fd8d26221db1d2f75ef84793d2cbd12a87e0185) feat(parser): parse GRANT and REVOKE for MCP temp tables
- [23b168d](https://github.com/jspreddy/dql/commit/23b168dec9dbe1a3c24f7d96a3b84a3a85155e6b) docs(plan): make the MCP temp table grants plan a self-contained handoff
- [348865a](https://github.com/jspreddy/dql/commit/348865a07d79788fdf5751da5d5cfd86a944381a) docs(plan): grant MCP read access to temp tables, in memory
- [d28fd15](https://github.com/jspreddy/dql/commit/d28fd15c449f931abc46f5b227976a7f05ee495c) fix(web): keep query-result ids inside one connection ([#273](https://github.com/jspreddy/dql/pull/273))
- [4e30e25](https://github.com/jspreddy/dql/commit/4e30e259c1a256ab43938afaac3fe60aebe5e94e) test(engine): cover chained joins and large in-memory equijoins ([#261](https://github.com/jspreddy/dql/pull/261))
- [52ba8d6](https://github.com/jspreddy/dql/commit/52ba8d642c90d991a6c5f26345431c4b4064cb25) fix(engine): satisfy clippy in the in-memory join test
- [635a2c4](https://github.com/jspreddy/dql/commit/635a2c4e33024869e56e4471b45b25df65d97fee) feat(engine): join rows loaded once by WITH or a temp table
- [ca36d32](https://github.com/jspreddy/dql/commit/ca36d3274ec64ce40e64d05362356ac9e5a63dce) docs(readme): shorten the examples and DynamoDB Local sections, after the screenshots ([#269](https://github.com/jspreddy/dql/pull/269))
- [d971622](https://github.com/jspreddy/dql/commit/d97162295dbafe34d9bcb61479abe0b44a0e7e9d) feat(web): drop tooltips that only repeat what the item shows ([#266](https://github.com/jspreddy/dql/pull/266))
- [8132b2c](https://github.com/jspreddy/dql/commit/8132b2c2aa0ec3fec3c787bd196344a95d23b44f) feat(web): label the table source input group Index ([#265](https://github.com/jspreddy/dql/pull/265))
- [a7bbd5a](https://github.com/jspreddy/dql/commit/a7bbd5a29a49c082611d7b75ad7d1ce577cfee05) test(web): the source's border and radius belong to its input group
- [18474fb](https://github.com/jspreddy/dql/commit/18474fb401cfa04482ceb7f3d46d4578f878e4ac) feat(web): table source as an "On [ -- ]" input group; -- adds nothing
- [494d288](https://github.com/jspreddy/dql/commit/494d288cba91a26c0b59c905993115a4462ed86e) fix(web): give table query dropdowns one style ([#260](https://github.com/jspreddy/dql/pull/260))
- [718a74a](https://github.com/jspreddy/dql/commit/718a74aac1d70b604841f34638e2fcff4e0df63b) fix(web): separate table list group titles with space and a rule ([#259](https://github.com/jspreddy/dql/pull/259))
- [a2149da](https://github.com/jspreddy/dql/commit/a2149dac597362b123ada900f1f42566ad4636d9) feat(web): dqlrs web --local brings up DynamoDB Local and serves against it ([#257](https://github.com/jspreddy/dql/pull/257))
- [517000f](https://github.com/jspreddy/dql/commit/517000f3904a7d550aa9de5489041ae63eaffa38) feat(cli): color dqlrs dynamo's output like the rest of the CLI
- [d9ebf13](https://github.com/jspreddy/dql/commit/d9ebf1342f1dfc6d9a9393e13cc7c928ac8ec9d3) test(cli): expect the shared-download Java plan line for --init-examples
- [07ce655](https://github.com/jspreddy/dql/commit/07ce6556cb5eadf0968452623f423842d1fc0043) docs: dqlrs dynamo in the README and rust-docs
- [f247f4e](https://github.com/jspreddy/dql/commit/f247f4e87c584179a94cb1839b6f63dd1a377488) feat(cli): dqlrs dynamo to list, start, stop, clean, and read DynamoDB Local logs
- [9325a89](https://github.com/jspreddy/dql/commit/9325a890d5efc158721a7255a2ab7b9b901f1eca) feat(changelog): one flat list of commits, PR numbers on each PR's latest commit ([#256](https://github.com/jspreddy/dql/pull/256))
- [703e83f](https://github.com/jspreddy/dql/commit/703e83f36fc5db4727be24773fe5e7a4fd208d87) fix(changelog): list a pull request's commits newest first, like the rest
- [d86c004](https://github.com/jspreddy/dql/commit/d86c004acc2325788f2303f82adebd3bf805483a) feat(changelog): list every commit a pull request brought in, grouped by stacked PR
- [805e079](https://github.com/jspreddy/dql/commit/805e0797a5cb2c825064074984dbeb38b03f5a77) fix(web): hand tooltips off at once within 300 ms of the last one hiding ([#254](https://github.com/jspreddy/dql/pull/254))
- [493174e](https://github.com/jspreddy/dql/commit/493174e55e49d0897a2b5084c4ce75a3a23f5337) feat(web): rank tooltips by obviousness too; tests and DESIGN.md notes
- [1314b58](https://github.com/jspreddy/dql/commit/1314b5859dc31e92be0d2c2d525b3edd2405b06a) feat(web): delay tooltips 100, 200, or 400 ms by how much they matter (WIP)
- [acf0aeb](https://github.com/jspreddy/dql/commit/acf0aebeba447e64cba42cac9f7b94a4f9f45fdf) test(web): allow for the Run bar's padding in the table pager layout check ([#253](https://github.com/jspreddy/dql/pull/253))
- [2106a9b](https://github.com/jspreddy/dql/commit/2106a9beaae3b977ef3d5336e6e57429637e7ae3) test(web): check the table query's Run bar sits on the panel's bottom edge when taller
- [5ab4cff](https://github.com/jspreddy/dql/commit/5ab4cffa014f5732ce35ee16edda1a65ff0433ba) feat(web): put the table query's Run bar under both panes, full width
- [13dc478](https://github.com/jspreddy/dql/commit/13dc47821da48a89a6c47c09812f78c19dcc2f3e) feat(web): resize the table query panel's height from its bottom edge
- [0d13d6e](https://github.com/jspreddy/dql/commit/0d13d6eaedb492f41aaab81fe125d4aea5c71bd6) feat(web): cap the table query's right pane at 55% of the panel
- [0977b6f](https://github.com/jspreddy/dql/commit/0977b6f6578bf9905ecf22910ff5af3aab29fc41) feat(web): plain explain steps in the table pane, JSON box on its tab bar
- [edcf2f0](https://github.com/jspreddy/dql/commit/edcf2f0c4d7d74dcfa4e2cf5fcac0bd53468889f) test(web): scope the side grip check to the main side divider
- [68cb88f](https://github.com/jspreddy/dql/commit/68cb88faa2f28b90e8605b60e7cdd375c54dfe39) feat(web): toggle and drag-to-hide for the table query's right pane
- [149b6b5](https://github.com/jspreddy/dql/commit/149b6b5e518c1e98137a45f01e7e045f0e4b3f12) feat(web): split the table query panel into controls and query/explain tabs
- [6736e15](https://github.com/jspreddy/dql/commit/6736e15ae6c8c4207c3b9af7e4585f02728a75db) feat(web): show Cancel, then rows, capacity, and time, next to the table Run ([#252](https://github.com/jspreddy/dql/pull/252))
- [cf43e5d](https://github.com/jspreddy/dql/commit/cf43e5dcd77a7f73eb4949036a582f51187eb839) feat(web): put the table browser's Run and pager in one bar under the DQL
- [ac78b7c](https://github.com/jspreddy/dql/commit/ac78b7c93f6f60f0c02de84c75c2ce0ca35a062d) chore(just): add dynamo-fresh (dd-fresh) to restart DynamoDB Local clean ([#251](https://github.com/jspreddy/dql/pull/251))
- [ca02383](https://github.com/jspreddy/dql/commit/ca0238300eec34b4f35442572b03cbd9177ec240) feat(web): move the run dialog's actions under its title, with shortcuts ([#250](https://github.com/jspreddy/dql/pull/250))
- [281a2cb](https://github.com/jspreddy/dql/commit/281a2cbce6d6634f9bfaab4f2159db3fab08050a) test(web): read the query editor's font, not the first CodeMirror on the page ([#249](https://github.com/jspreddy/dql/pull/249))
- [a6fe359](https://github.com/jspreddy/dql/commit/a6fe359e3c725d648a8d84027b762109c012d6a0) feat(web): highlight keys in the table browser's Keys box
- [64d5cf0](https://github.com/jspreddy/dql/commit/64d5cf057292d1d34bb195c89f3d11568ba9c2ab) test(web): pin down repeated (hash, range) tuples in the Keys box ([#248](https://github.com/jspreddy/dql/pull/248))
- [895ef18](https://github.com/jspreddy/dql/commit/895ef18d3236c0c5a1068705ffbb42c7c4e51543) feat(web): leave repeated keys out of the Keys box statement
- [17843f0](https://github.com/jspreddy/dql/commit/17843f0cd66743870ad26a1826fda9de5d959ef2) test(web): screenshot the key format in the query editor and table viewer ([#244](https://github.com/jspreddy/dql/pull/244))
- [3823720](https://github.com/jspreddy/dql/commit/382372003dde37f93f42d0391764e5ac7011804a) docs(help): keep the key format paragraph prose in HELP SELECT
- [2a35d86](https://github.com/jspreddy/dql/commit/2a35d86ca66b74d816695cec22fc6d8dc14ea5e4) feat: one key format for KEYS IN, WHERE ... IN, and the Keys box
- [dd69e8c](https://github.com/jspreddy/dql/commit/dd69e8c99beee52c0f9771a5e05033a2eedcabcf) fix(web): accept a list of hash keys in Keys mode on range-key tables ([#242](https://github.com/jspreddy/dql/pull/242))
- [c918cda](https://github.com/jspreddy/dql/commit/c918cda6f893630dee5f8d566ea923fb82d5e2f8) fix(web): check key values against the key type for every table
- [58e6aa1](https://github.com/jspreddy/dql/commit/58e6aa1fc0e6974145179788b1c7ac8e865154f7) fix(web): keep the table source dropdown a fixed width ([#241](https://github.com/jspreddy/dql/pull/241))
- [dc3b15a](https://github.com/jspreddy/dql/commit/dc3b15ae8fa727da3cb407b97fd8b13f4a3ddbab) fix(web): keep the table source beside Scan, Query, and Keys
- [6343bd4](https://github.com/jspreddy/dql/commit/6343bd4f5c94b7fe67ece00a6ef36ee1fc831ce8) Show the just verify summary unchanged ([#243](https://github.com/jspreddy/dql/pull/243))
- [7c3d84f](https://github.com/jspreddy/dql/commit/7c3d84f3a25e8434923715aaed282d8da941cdc4) docs: drop the post-PR just verify workflow step ([#237](https://github.com/jspreddy/dql/pull/237))
- [27d09c0](https://github.com/jspreddy/dql/commit/27d09c0f7b3a5166d7b224ab7f4ffd5f621dd5b4) feat(web): run DynamoDB condition functions from the table picker ([#238](https://github.com/jspreddy/dql/pull/238))
- [e79691c](https://github.com/jspreddy/dql/commit/e79691c978ab2d1b1515c3104c62a66513bbff00) feat(web): serve table pager turns from memory until Run ([#235](https://github.com/jspreddy/dql/pull/235))
- [4cb0fc8](https://github.com/jspreddy/dql/commit/4cb0fc8cfe0c11174d1dc5f9df35f0aec016e211) test(web): walk a thousand numeric ids through the table pager ([#234](https://github.com/jspreddy/dql/pull/234))
- [62760b9](https://github.com/jspreddy/dql/commit/62760b981c5ca5587e05eb7d88e628e501f7553b) fix(web): fetch one extra table row so the last page disables Next
- [bc994b0](https://github.com/jspreddy/dql/commit/bc994b02d0f513321f4e4ca81f3d91c24c510831) fix(web): put table pagination in the query toolbar ([#232](https://github.com/jspreddy/dql/pull/232))
- [6e22729](https://github.com/jspreddy/dql/commit/6e227293f21b4f3fc6cb15b72484c1b0c65f2805) Accept grouped KEYS IN lists and trailing commas. ([#225](https://github.com/jspreddy/dql/pull/225))
- [fa2e6fa](https://github.com/jspreddy/dql/commit/fa2e6fafe166df4a08f213d3f8538c56ecc6a8cf) Let Keys mode pick TABLE or an index.
- [ba3183a](https://github.com/jspreddy/dql/commit/ba3183a59de53f5f59aa8ae508e5b4267a9f1052) Require parentheses for hash, range key tuples.
- [141c323](https://github.com/jspreddy/dql/commit/141c3235f9038fa5ad7cf192d1b428e0022b2cd3) Add a Keys mode to the table browser for KEYS IN lookups.
- [465a009](https://github.com/jspreddy/dql/commit/465a0097fc439d022f292e6ea44bb591ef2d7a32) Suggest table attributes in the query picker, and allow typed names. ([#223](https://github.com/jspreddy/dql/pull/223))
- [e2c586e](https://github.com/jspreddy/dql/commit/e2c586eb9fa5b2a563979fc51d2260ed23231a11) Add a scan and query picker to the table browser.
- [8dd5cbe](https://github.com/jspreddy/dql/commit/8dd5cbe24cede53e705ccca3ed0c4627b486d5cc) test(notebook): say what the page showed when a cell does not finish ([#228](https://github.com/jspreddy/dql/pull/228))
- [ec4697e](https://github.com/jspreddy/dql/commit/ec4697ee31722e22a972efc43bdea4c568882c01) test(web): read only the new result card in the ST listing test ([#227](https://github.com/jspreddy/dql/pull/227))
- [c91b43c](https://github.com/jspreddy/dql/commit/c91b43c168d1c878329fc499c088595504f5bcc1) docs(web): plan splitting app.js into modules
- [a1306a6](https://github.com/jspreddy/dql/commit/a1306a6e192f1bdce02aa080cdec58f4266896dc) fix(web): bundle Noto Sans so the app works offline
- [a909ca7](https://github.com/jspreddy/dql/commit/a909ca77680e3b66cc2c0411a8a4d69441b46002) refactor(web): share button and dialog components; add design guidelines
- [01c2309](https://github.com/jspreddy/dql/commit/01c2309ca8e828f893f48cfbfb05de822f413536) docs: merge the Conventional Commits rule into the commit rule ([#221](https://github.com/jspreddy/dql/pull/221))
- [4a9f7ae](https://github.com/jspreddy/dql/commit/4a9f7ae174f55ec1c2846d105bc3cb9d6c02534a) docs: add a Conventional Commits rule
- [ab341f7](https://github.com/jspreddy/dql/commit/ab341f705d56ed3d368725921b2c5702247966b1) Link release tags to their changelog heading; date headings by their newest change ([#220](https://github.com/jspreddy/dql/pull/220))
- [2c38614](https://github.com/jspreddy/dql/commit/2c38614fc61db4674ee7fc1c6a4bf4404836ffe9) Fail the cache key step when no dependencies are found ([#218](https://github.com/jspreddy/dql/pull/218))
- [c93262f](https://github.com/jspreddy/dql/commit/c93262f59216a7b02226b09eb33886b796d7945e) Key the cargo caches on dependencies, not the whole lockfile
- [c6d0f40](https://github.com/jspreddy/dql/commit/c6d0f408d83f54291a6cba071114a45d994de094) Cache cargo in CI and install cargo-release from its release binary

## dqlrs 0.7.11 (2026-11-15)

Contributors: [Sai Jonnala](https://github.com/jspreddy)\
Agents: Claude Opus 5.5

- [41a9860](https://github.com/jspreddy/dql/commit/41a986078209653f70813009920682edad4c8f80) Show the v-rust install next to the release install in the README summary ([#215](https://github.com/jspreddy/dql/pull/215))
- [f828ba4](https://github.com/jspreddy/dql/commit/f828ba4aa0d44ae0371f47e230dbcc61d58cb759) Say that v-rust is the GitHub default branch
- [425686f](https://github.com/jspreddy/dql/commit/425686fae90d09be8a18eed83ee0aaca3a397313) Group the release flow files into read and written
- [86ba2b1](https://github.com/jspreddy/dql/commit/86ba2b1aaa7c55cf0e5c79ee0dcc6aca6cb0cd29) Group the release flow entry points by GitHub Actions and manual
- [a0ecb92](https://github.com/jspreddy/dql/commit/a0ecb923c34b90af70b111e42a16400b21ffa00a) Remove the changelog skill
- [f8d7e2d](https://github.com/jspreddy/dql/commit/f8d7e2dd63379980fcd504682dc2ebbb0a424ddf) Remove the uv run task bump entry point
- [ce9657f](https://github.com/jspreddy/dql/commit/ce9657f5a14c175b6e104973320c632cfe5526eb) Draw the release flow graph with direct lines
- [46c0b94](https://github.com/jspreddy/dql/commit/46c0b94235972742e232de628afeac1cc83f23e4) Add a graph of the versioning and release flow
- [83d165a](https://github.com/jspreddy/dql/commit/83d165ad19d5c266ecc90c5c12ac66691b9503f7) Pin README install tags with cargo-release replacements
- [7c2da40](https://github.com/jspreddy/dql/commit/7c2da40e3269e41f57965b8384bd94dd9db383bc) Keep the release changelog equal to what v-rust writes
- [829e443](https://github.com/jspreddy/dql/commit/829e443a22944b3fdcb81c8e3b75dff5e532aa13) Add a Release dqlrs workflow and release install commands
- [53a61d1](https://github.com/jspreddy/dql/commit/53a61d183514cc3231eb0da048384287f3b4c779) Remove the empty py-plans folder ([#213](https://github.com/jspreddy/dql/pull/213))
- [c193040](https://github.com/jspreddy/dql/commit/c19304061242cbcedf9c2c2fc18f08da89656104) Fold MEMORY.md into AGENTS.md ([#209](https://github.com/jspreddy/dql/pull/209))
- [ff3b3fa](https://github.com/jspreddy/dql/commit/ff3b3fa2236eedb8c02573003b4d4b2ed2a94fe3) Add MEMORY.md with pointers for new agents

## dqlrs 0.7.10 (2026-11-14)

Contributors: [Sai Jonnala](https://github.com/jspreddy)\
Agents: Claude Opus 5.5

- [2c0c74d](https://github.com/jspreddy/dql/commit/2c0c74db8548f88f8f606d79c1ce1ff2af02fba8) Bump the Rust version with cargo-release and semver pre-releases ([#210](https://github.com/jspreddy/dql/pull/210))

## dqlrs 0.7.10-dev0 (2026-11-13)

Contributors: [Sai Jonnala](https://github.com/jspreddy)\
Agents: Claude Opus 5.5, Cursor Agent

- [0a76795](https://github.com/jspreddy/dql/commit/0a76795deb468e4d3af7b40f4e770523ef370aef) Bump version: 0.7.9-dev0 → 0.7.10-dev0 ([#206](https://github.com/jspreddy/dql/pull/206))
- [01e3a59](https://github.com/jspreddy/dql/commit/01e3a59be4b4a01c4fc6a7c1105b699313b35f13) Do not link pull requests merged before 2024-10-01 in the changelog ([#203](https://github.com/jspreddy/dql/pull/203))
- [da5d250](https://github.com/jspreddy/dql/commit/da5d250d4e4077c27dd586b4a6e9ce4e5340fd2f) Document HELP and LS in the README's supported queries ([#202](https://github.com/jspreddy/dql/pull/202))
- [17f975d](https://github.com/jspreddy/dql/commit/17f975d4f2f2120db3f6096443fe2d7e32e04587) Show Sai Jonnala for every jspreddy alias in the changelog. ([#198](https://github.com/jspreddy/dql/pull/198))
- [d6e5b44](https://github.com/jspreddy/dql/commit/d6e5b44540c051b005f1cea27772241a47f075a2) Put changelog contributors on one line under each version heading.
- [0c5df1f](https://github.com/jspreddy/dql/commit/0c5df1f12143c98ecbda7d140ad34847673f9914) Map Sai's author names to the jspreddy GitHub user.
- [8c8a356](https://github.com/jspreddy/dql/commit/8c8a35603aa0d88efd6749065d19485ac6f75232) List contributors under each changelog version.
- [bf75825](https://github.com/jspreddy/dql/commit/bf75825e20e85bda5257205603b2e72278cde6d2) Link each changelog hash at the start of the line.
- [9e5979c](https://github.com/jspreddy/dql/commit/9e5979c064faefcb09ea0aa73f68faa849b7c0b4) Show the commit hash on every changelog entry.
- [9f420c1](https://github.com/jspreddy/dql/commit/9f420c13b690066d05dac1b748476fa82aa291bf) Run the changelog skill through just.
- [7aeb92a](https://github.com/jspreddy/dql/commit/7aeb92a2b4aee3c56ed9523f6836df373134ded9) Group the changelog by Bump version commits.
- [fe1f244](https://github.com/jspreddy/dql/commit/fe1f24484d99296c15b88d9a2a85d16f6fc9bc2f) Move --init-examples to the third row of the banner commands ([#199](https://github.com/jspreddy/dql/pull/199))
- [2138c62](https://github.com/jspreddy/dql/commit/2138c6204ad82a2cb85cd3268717fbc6e7605b5d) Add dqlrs web --init-examples to the README banner

## dqlrs 0.7.9-dev0 (2026-11-13)

Contributors: [Sai Jonnala](https://github.com/jspreddy)\
Agents: Claude Opus 5.5, <a href="https://github.com/dependabot%5Bbot%5D">dependabot[bot]</a>

- [82e0cdb](https://github.com/jspreddy/dql/commit/82e0cdb284079fa9d4d965b16d7f15f9f9fafbe4) Use more colors: headings, JSON, URLs, paths, variables, shortcuts ([#196](https://github.com/jspreddy/dql/pull/196))
- [b79eb01](https://github.com/jspreddy/dql/commit/b79eb01ebb09d2d07c04d82660f22d8fca092aca) Color what the dqlrs command prints to the console
- [3614c6b](https://github.com/jspreddy/dql/commit/3614c6b7c59285cd864dc9641d3cd7364855a69a) Set up DynamoDB Local in dqlrs web --init-examples ([#195](https://github.com/jspreddy/dql/pull/195))
- [92c51e3](https://github.com/jspreddy/dql/commit/92c51e34e3e18b022dd52f3c496b93aa083dfea2) Say git must be properly installed and ready to commit ([#194](https://github.com/jspreddy/dql/pull/194))
- [50f97b0](https://github.com/jspreddy/dql/commit/50f97b018567c51e42d4fc6559487b4d4169c921) Drop the git name and email check from the --init-examples docs
- [8b20598](https://github.com/jspreddy/dql/commit/8b205980516b52a67ab087e1cc2138fea1765b29) Show the connection and role above the table definition progress ([#192](https://github.com/jspreddy/dql/pull/192))
- [cd08eaf](https://github.com/jspreddy/dql/commit/cd08eaf8c4119ab77d75933dd8469e7b7be612b0) Say "table definitions" and show only the bar while loading
- [cd478eb](https://github.com/jspreddy/dql/commit/cd478eba0cfab0c0ec7320638854c4a7f4d7beba) Load tables with progress before dqlrs web starts serving
- [923e712](https://github.com/jspreddy/dql/commit/923e7122ca8611b9e4b58f66f8ac1ffb2a07505d) Show step progress while dqlrs web --init-examples runs ([#191](https://github.com/jspreddy/dql/pull/191))
- [77586bc](https://github.com/jspreddy/dql/commit/77586bc41e72451c38d7177645cf18265bc09788) Use one flag, --init-examples, instead of --init and --examples
- [0fa9b5b](https://github.com/jspreddy/dql/commit/0fa9b5b4bfa279a8a0c0f79cd54200c1f59ade41) Add dqlrs web --init --examples to start a workspace from the examples
- [2a9ceff](https://github.com/jspreddy/dql/commit/2a9ceffbd3f6e057705a6648f38aebf2bb561b20) Replace dead diagram links in the example READMEs ([#190](https://github.com/jspreddy/dql/pull/190))
- [201d818](https://github.com/jspreddy/dql/commit/201d81804828dc7741c64493517336faa9b07a50) Fix the vulnerabilities cargo audit and pip-audit report ([#187](https://github.com/jspreddy/dql/pull/187))
- [9daf1af](https://github.com/jspreddy/dql/commit/9daf1afcf366cb4d5252bc1df5d1db021586e761) Add a filetype-report skill that renders a PNG of file types in git. ([#186](https://github.com/jspreddy/dql/pull/186))
- [0eaccd3](https://github.com/jspreddy/dql/commit/0eaccd38e88b43c77ec8beab57862d4347ae5304) Bump certifi from 2026.7.22 to 2026.10.18 in /notebook ([#184](https://github.com/jspreddy/dql/pull/184))
- [cc148a3](https://github.com/jspreddy/dql/commit/cc148a3c478d3a6315ae329829a94199ee32986c) Bump certifi from 2026.7.22 to 2026.10.18 in /notebook-tests ([#181](https://github.com/jspreddy/dql/pull/181))
- [e13aea6](https://github.com/jspreddy/dql/commit/e13aea6afec04080697b3b45a014efc7721abe71) Bump certifi from 2025.7.14 to 2026.10.18 in /py-impl ([#179](https://github.com/jspreddy/dql/pull/179))
- [d592387](https://github.com/jspreddy/dql/commit/d592387986490b15e42006c4e695994a41fe58bf) Run just check and update the changelog in GitHub Actions ([#175](https://github.com/jspreddy/dql/pull/175))

## dqlrs 0.7.8-dev0 (2026-11-12)

Contributors: [Sai Jonnala](https://github.com/jspreddy)\
Agents: Claude Opus 5.5

- [cffaff7](https://github.com/jspreddy/dql/commit/cffaff750dc25a5225a2d9881c8d6a516d6266e4) Bump dqlrs to 0.7.8-dev0 and update the changelog ([#173](https://github.com/jspreddy/dql/pull/173))
- [26f0267](https://github.com/jspreddy/dql/commit/26f026729a29de17697f7fad59326468e7c118d6) Use JupyterLab Dark for the README notebook screenshots. ([#170](https://github.com/jspreddy/dql/pull/170))
- [4decc41](https://github.com/jspreddy/dql/commit/4decc414d12dc42c4bc23b53b67186355386b804) Use the dark theme for the README web screenshots.
- [eb234a3](https://github.com/jspreddy/dql/commit/eb234a371094f228e9af93285411171e3f201c4f) Group Documentation and About this fork under a Maintainer section.
- [244149a](https://github.com/jspreddy/dql/commit/244149a5ee280d05fba23679fb6cbf5385046300) Point the aws-vault links at the ByteNess fork.
- [388c315](https://github.com/jspreddy/dql/commit/388c315740884b865d4ed11a12367d2f6c63fcc2) Link the README summary to each surface and mention aws-vault.
- [6f194d6](https://github.com/jspreddy/dql/commit/6f194d6f5692be9e6d40ddc410f5293e46212f5d) Offer only git-URL installs until there are releases.
- [41ee90f](https://github.com/jspreddy/dql/commit/41ee90f0566254830ffb0909baa48d5842dad3ca) Update docs and screenshots for dqlrs web and the workspace cache.
- [1a49d80](https://github.com/jspreddy/dql/commit/1a49d80b02b774cb13e682a06b588564b4297680) Mark the security plans as shipped.
- [d8964c7](https://github.com/jspreddy/dql/commit/d8964c718f91d664756a4af11a7bb45530184102) Move the security plans into the plans archive.
- [8ff5287](https://github.com/jspreddy/dql/commit/8ff52876cdf6c0ba02f74d9624cdab14ca692bae) Lead the README with a banner and a quick pitch.
- [4cd739c](https://github.com/jspreddy/dql/commit/4cd739c4f9dd302daeb865a0b57d6381df52a3d4) Refresh README screenshots and update the README.
- [6e9ce09](https://github.com/jspreddy/dql/commit/6e9ce09a91808df1ce0bcce6536f617d5f20fd9c) Index rust-docs and fix links after the plans move.
- [06605e0](https://github.com/jspreddy/dql/commit/06605e04ef9c7be290674a2e1e3a81b116b78990) Move Rust plans and archived Cursor plans under rust-docs.

## dqlrs 0.7.7-dev0 (2026-11-12)

Contributors: [Sai Jonnala](https://github.com/jspreddy)\
Agents: Claude Opus 5.5, Cursor

- [7647ea3](https://github.com/jspreddy/dql/commit/7647ea3784c0ae729dbfcd703fd882c81206cb09) Make the listed-table search test create its own table ([#172](https://github.com/jspreddy/dql/pull/172))
- [c7a5af2](https://github.com/jspreddy/dql/commit/c7a5af2faa28a3d60cd2de546d4952232ad8cbeb) Fix web tests for canonical paths and closed-tab presence ([#171](https://github.com/jspreddy/dql/pull/171))
- [361b1d4](https://github.com/jspreddy/dql/commit/361b1d443539affcd1e1e8bf1a46992e40854468) Wait for the workspace watch before the web server accepts clients
- [9477025](https://github.com/jspreddy/dql/commit/94770251dacc338b2a65100a5b71f10f36091029) Make the web UI a subcommand: dqlrs web ([#169](https://github.com/jspreddy/dql/pull/169))
- [2c118cf](https://github.com/jspreddy/dql/commit/2c118cf1e58d55017884cf8cb2638287f36c370f) Make a bare ST pattern match anywhere in the name ([#168](https://github.com/jspreddy/dql/pull/168))
- [efae0cc](https://github.com/jspreddy/dql/commit/efae0cc94f54d70fb4f02a5b75482fc0d4931415) Let ST take a quoted or bare pattern ([#167](https://github.com/jspreddy/dql/pull/167))
- [57d2f31](https://github.com/jspreddy/dql/commit/57d2f31dec173f7c139e2f731ace7422c568f09a) Add ST as short for SHOW TABLES ([#166](https://github.com/jspreddy/dql/pull/166))
- [b4d6b84](https://github.com/jspreddy/dql/commit/b4d6b84497fd3ee702d49a0384b58cb170673bb8) Add a generated CHANGELOG.md and a changelog skill ([#165](https://github.com/jspreddy/dql/pull/165))
- [e5de8bd](https://github.com/jspreddy/dql/commit/e5de8bd1bdf78419fc3e619fc3f18b982aedd1e4) Extract reusable web components from app.js, with component tests ([#164](https://github.com/jspreddy/dql/pull/164))
- [36fe6ca](https://github.com/jspreddy/dql/commit/36fe6caaa63463c3d227dcade2283d8814f1e1b8) Measure editor lines until they have a box in web tests ([#163](https://github.com/jspreddy/dql/pull/163))
- [700445f](https://github.com/jspreddy/dql/commit/700445f6bc3da876388f30b0f6dfaf997a5276ea) Confirm and track tree-menu runs in a full-screen run dialog ([#162](https://github.com/jspreddy/dql/pull/162))
- [6d689df](https://github.com/jspreddy/dql/commit/6d689df1364563defe1795431356cfc06749144e) Clear local tables and the result cache from the settings menu ([#161](https://github.com/jspreddy/dql/pull/161))
- [d065595](https://github.com/jspreddy/dql/commit/d06559513963c8a157fdf94c8d9ea3d3c50f83af) Run test suites' dqlrs outside the repository ([#160](https://github.com/jspreddy/dql/pull/160))
- [ab2965e](https://github.com/jspreddy/dql/commit/ab2965e8d43438c273ca738c8a0fcf2991b07308) Fix the local table example paths for the ./ rule ([#158](https://github.com/jspreddy/dql/pull/158))
- [66145e5](https://github.com/jspreddy/dql/commit/66145e5e7ea8bdbce2aa5c147e911787aeb1b9cf) Keep all dqlrs data in <workspace>/.dqlrs-cache, per connection ([#157](https://github.com/jspreddy/dql/pull/157))
- [60a32d6](https://github.com/jspreddy/dql/commit/60a32d6950dbdb349f136de9aeca89e2716dd8bf) Use local and temp table definitions in the linter and structure view ([#155](https://github.com/jspreddy/dql/pull/155))
- [ccfe284](https://github.com/jspreddy/dql/commit/ccfe284838fe03028d530770dad9176c0578ec3d) Keep comments out of lint underlines ([#154](https://github.com/jspreddy/dql/pull/154))

## dqlrs 0.7.6-dev0 (2026-11-11)

Contributors: [Sai Jonnala](https://github.com/jspreddy)\
Agents: Claude Opus 5.5, Cursor Agent

- [19976a9](https://github.com/jspreddy/dql/commit/19976a9bb9f38eab1250f66d4e56fd5c6664a376) Bump Rust version: 0.7.5-dev0 → 0.7.6-dev0 ([#153](https://github.com/jspreddy/dql/pull/153))
- [d47befd](https://github.com/jspreddy/dql/commit/d47befd18eb229a8c3c9c1b59ab52f0461ee7123) Keep a folder open when a pending folder click fires ([#152](https://github.com/jspreddy/dql/pull/152))
- [397655b](https://github.com/jspreddy/dql/commit/397655b6b446c53af3f0297f08688105d77b56ab) Resolve file paths in a .dql file against the file or the workspace ([#151](https://github.com/jspreddy/dql/pull/151))
- [b213103](https://github.com/jspreddy/dql/commit/b21310373196670ad0de51d43a26d8fb40e7e5dd) Scroll the table structure dialog as one surface ([#150](https://github.com/jspreddy/dql/pull/150))
- [1b38143](https://github.com/jspreddy/dql/commit/1b38143e17a2f01b8a151d3ebdb41314af1a076c) Address review of the table structure view ([#148](https://github.com/jspreddy/dql/pull/148))
- [f91a29d](https://github.com/jspreddy/dql/commit/f91a29d9f8a304c88a5c207a8f5f07597f67f4f2) Name load and save results. ([#142](https://github.com/jspreddy/dql/pull/142))
- [b0a5607](https://github.com/jspreddy/dql/commit/b0a5607d1a7882e359d871c57014d145c451ffbd) Name insert, update, delete, count, and drop results.
- [91cf4ec](https://github.com/jspreddy/dql/commit/91cf4ec867897fade5e2888f9427f87d7f381133) Add a shared note card with error, success, alert, and info variants.
- [eb11464](https://github.com/jspreddy/dql/commit/eb1146445059d4465ecdad55bda21d1b5a268e0c) Style the result structure note as a note card.
- [5aa953a](https://github.com/jspreddy/dql/commit/5aa953a4f60e0102b69bd7055c64c94fd91a6018) Note that result structure is partial and drop the table footer line.
- [1923bf0](https://github.com/jspreddy/dql/commit/1923bf0adae2cfb795f6c835f44999e6c35c61d3) Group the create query and column table under Definition.
- [d77440a](https://github.com/jspreddy/dql/commit/d77440a958dba73a6a7234fcd5165085a3570276) Show the table definition as columns next to the formatted create query.
- [58bac4f](https://github.com/jspreddy/dql/commit/58bac4ffc04e195bccaf5b99560bddc2698b2f1d) Show the cached schema in the table structure view.
- [847a446](https://github.com/jspreddy/dql/commit/847a446f7e437a3b0f5d521884e8e40b8d8da52a) Keep a tree rename open when the tree repaints ([#147](https://github.com/jspreddy/dql/pull/147))
- [3842cc2](https://github.com/jspreddy/dql/commit/3842cc216336d715b1d79d346b859f895d80bdf5) Make the file-tree drag test deterministic
- [21c18ed](https://github.com/jspreddy/dql/commit/21c18ed9017b7e3806274d08af480e09479ccd2b) Fix two racy web tests ([#146](https://github.com/jspreddy/dql/pull/146))
- [0609fbc](https://github.com/jspreddy/dql/commit/0609fbcdb3500818a2723dee2e5c9048d01e4df2) git-graph skill: always output a PNG ([#144](https://github.com/jspreddy/dql/pull/144))
- [fc82f8b](https://github.com/jspreddy/dql/commit/fc82f8b021b706dc3b8ccc20e382445ee9b49b39) Add branching rule: v-rust is the target branch, use stacked PRs ([#143](https://github.com/jspreddy/dql/pull/143))
- [a4b2ccc](https://github.com/jspreddy/dql/commit/a4b2cccea7d00d0a207753f47353d8565d5b56f6) notebook-tests: pass --allow-root to JupyterLab when running as root
- [652b4e8](https://github.com/jspreddy/dql/commit/652b4e8b9ae0192cd04f9e31b2790c1db2a67a9d) Use per-file .md symlinks in .claude/rules for Cursor .mdc rules
- [7d07a86](https://github.com/jspreddy/dql/commit/7d07a86f99de74629cd4cc02503fd40e1b2f8c0e) Expose .cursor/rules to Claude Code via .claude/rules symlink
- [2e90233](https://github.com/jspreddy/dql/commit/2e90233202db2b1b64dbb8509966beb6b8cd6825) Expose .cursor/skills to Claude Code via .claude/skills symlink

## dqlrs 0.7.5-dev0 (2026-11-08)

Contributors: [Sai Jonnala](https://github.com/jspreddy)\
Agents: Cursor Agent

- [38356b8](https://github.com/jspreddy/dql/commit/38356b87aee85028aa1a9259b6219cca9e123f40) Show the full path in the delete confirm dialog. ([#141](https://github.com/jspreddy/dql/pull/141))
- [38f233b](https://github.com/jspreddy/dql/commit/38f233bd40cb3b66b5d95366161687ebd1fd8985) Replace window.prompt with an in-app input dialog.
- [350fc8d](https://github.com/jspreddy/dql/commit/350fc8d2939ca59b5d49c899e86c1964bceeee31) Replace window.confirm with an in-app delete dialog.
- [9731c8b](https://github.com/jspreddy/dql/commit/9731c8bb5b6b4fca1730e7272b42a5ef65c27fa4) Use Mermaid's built-in flowchart label wrapping. ([#139](https://github.com/jspreddy/dql/pull/139))
- [abab9b0](https://github.com/jspreddy/dql/commit/abab9b015e81fa8fdafe6ae5957169b2128282ae) cealnup diagram.html files in favor of markdown render for mermaid
- [c5294ba](https://github.com/jspreddy/dql/commit/c5294ba26ea355bc12e837a5372c1c6e6acee857) Wrap Mermaid node labels so the box grows with the text.
- [ff10515](https://github.com/jspreddy/dql/commit/ff105157d11834ac6bcd8ee521e39d151359fd1b) Use Noto Sans for the web interface.
- [b3d79c6](https://github.com/jspreddy/dql/commit/b3d79c6b01f1e23a0b1f1528b320a8ff0ab0a55f) Restore node hover and click in the full-screen Mermaid view.
- [d6040ec](https://github.com/jspreddy/dql/commit/d6040ec31145b40ec56efcd61d60ca7e4bdc9327) Scale the full-screen Mermaid drawing with the zoom label.
- [ee06df9](https://github.com/jspreddy/dql/commit/ee06df943befe7a167dc6e283f9b76f0c208c3be) Let Mermaid full-screen zoom past the fitted scale.
- [cdf2101](https://github.com/jspreddy/dql/commit/cdf21012b4a79734e588bce237af866c79a54aa3) Zoom Mermaid full screen with the wheel and pan it by dragging.
- [6f311bd](https://github.com/jspreddy/dql/commit/6f311bd1bc3348b296d57bd04fae51d0f629627f) Color Mermaid diagrams from the active theme and open them full screen.
- [77fb9ed](https://github.com/jspreddy/dql/commit/77fb9ed5228702cc9949ac618c34e985c5e24560) Let markdown documents use the full pane width.
- [bef3b7b](https://github.com/jspreddy/dql/commit/bef3b7b10fbcba10c3f841d5851fd61054ba6d36) Use a dark glow on Mermaid nodes in dark mode.
- [c6885d3](https://github.com/jspreddy/dql/commit/c6885d3d0b6e5ddc6d88d551a7338dc81d00ddf1) Color nav file icons with the type inks.
- [1428ffa](https://github.com/jspreddy/dql/commit/1428ffac0b44aceb63a4b80e134c581d5e923087) Mark nav files and folders with outlined type icons.
- [e15fcb0](https://github.com/jspreddy/dql/commit/e15fcb0981d7990981d21f5423fa10cb7ad8d9ab) Color the file suffix in the nav tree and drop the type pill.
- [f851d97](https://github.com/jspreddy/dql/commit/f851d9727c8274f69694cc6dcfd703846deae32b) Show hover help with the custom tip instead of the browser tooltip.
- [91eb39e](https://github.com/jspreddy/dql/commit/91eb39e1ed25883935dba3b31400b3f588d1ce96) Match the group-by button border to the other controls.
- [7d6a443](https://github.com/jspreddy/dql/commit/7d6a443b39863a1685887161eaf92f25ec357e91) Color CSV like JSON and Markdown like HTML.
- [d216a25](https://github.com/jspreddy/dql/commit/d216a251cf9775c901a2b0e73cb1e01eb7678cd8) Use Material icons from the vendor folder.
- [b27569f](https://github.com/jspreddy/dql/commit/b27569fa36ec77b8dcadf1b70aa29718a08b3d38) Serve Mermaid from the web app's vendor scripts.
- [d7231f7](https://github.com/jspreddy/dql/commit/d7231f78f684e8085898cba5da1352fdf4dd0a8a) Render mermaid diagrams inside markdown files.
- [ffacce3](https://github.com/jspreddy/dql/commit/ffacce3217b95ef234de5feaf7dbe25ac4eafd46) Drop links to the removed AWS example JSON files.
- [b65b1ac](https://github.com/jspreddy/dql/commit/b65b1acabfeb4006544992d4476f90913b6a3a66) delete un-needed json files for aws examples
- [6880feb](https://github.com/jspreddy/dql/commit/6880feb1015de4757b187dfb0a35b0343219b90d) Put the file icons on the title row and the type pills below.
- [b493786](https://github.com/jspreddy/dql/commit/b493786165b0d0a9f246c6ce180b36795b560f4d) Start the file tree collapsed and add fold buttons.
- [1057cd4](https://github.com/jspreddy/dql/commit/1057cd46e64056e39f9045824d534d80e083b48c) Expect the file type filters in the side heading.
- [5d6489f](https://github.com/jspreddy/dql/commit/5d6489f1a4997d5ba9cb2f3aa646fe48d7f3ce79) Resolve document links from the open file and filter the tree by type.
- [eb21c18](https://github.com/jspreddy/dql/commit/eb21c186ce2fceab8293fcc595357185b229b9d1) Show markdown and HTML in the web UI, and run only .dql files from a folder.
- [a6383f2](https://github.com/jspreddy/dql/commit/a6383f2cbce8afc6a775547ecbaa5542900e52ae) Document the earlier example data models. ([#137](https://github.com/jspreddy/dql/pull/137))
- [28d9dc1](https://github.com/jspreddy/dql/commit/28d9dc105a9d6479e66fbd924c20286d2b3f74b0) Document the AWS design-pattern data models.
- [730cb48](https://github.com/jspreddy/dql/commit/730cb48563ce34a238717ababe9be4c5961cdfef) Add AWS DynamoDB design pattern examples.
- [f2b326b](https://github.com/jspreddy/dql/commit/f2b326b97bac85e470bb5e1eecee374567614eef) Add Harbor and Pine single-table examples.
- [c9effae](https://github.com/jspreddy/dql/commit/c9effae95186034200133c260853938b909be7bb) Add Sai Phaninder Reddy Jonnala's copyright to the MIT notice. ([#138](https://github.com/jspreddy/dql/pull/138))

## dqlrs 0.7.4-dev0 (2026-11-08)

Contributors: [Sai Jonnala](https://github.com/jspreddy)\
Agents: Cursor Agent

- [651de3d](https://github.com/jspreddy/dql/commit/651de3d80fb04d77cb4f2355b8dfe8d7ece29f8d) Raise the git graph default commit count to 100. ([#136](https://github.com/jspreddy/dql/pull/136))
- [687219e](https://github.com/jspreddy/dql/commit/687219ebd8a81a725cab848017ed7d510177a9fa) update git-graph skill and script
- [d92b6ca](https://github.com/jspreddy/dql/commit/d92b6ca396be6d9d5012868afbd16db614e25ff3) Keep git graph commit subjects under 80 characters.
- [6e0652f](https://github.com/jspreddy/dql/commit/6e0652fd3de8fabc2bb7d4f1d45b2a6eace3c50d) Choose git-graph branches with git and prune only when a cutoff is set.
- [942f2eb](https://github.com/jspreddy/dql/commit/942f2ebe91b5d46e8e06f1e8a3a3f85f18eea5fe) Let the git graph take a commit count and a cutoff.
- [ef4ae0c](https://github.com/jspreddy/dql/commit/ef4ae0c03fd10a739c51d749eedfcd02a2b3d2b4) Write each git graph to a new SVG file. ([#135](https://github.com/jspreddy/dql/pull/135))

## dqlrs 0.7.3-dev0 (2026-11-07)

Contributors: [Sai Jonnala](https://github.com/jspreddy)\
Agents: Cursor Agent

- [668aa18](https://github.com/jspreddy/dql/commit/668aa184d7102d16261d3dc6a8cec725aff88003) Wait for the tree event before asserting it. ([#134](https://github.com/jspreddy/dql/pull/134))
- [5d2de6c](https://github.com/jspreddy/dql/commit/5d2de6c7d6f8e5df8ec36ab694d4ed5a271d14ee) Log a lint failure and keep the editor edit. ([#133](https://github.com/jspreddy/dql/pull/133))
- [0f8d376](https://github.com/jspreddy/dql/commit/0f8d3763c89ba9ba64be101010d1eb580d733b14) Drop lint marks on edit so a bad range cannot reject a keystroke.
- [0254865](https://github.com/jspreddy/dql/commit/02548651714e8eb4d3397b67e1d1241ae543bfaf) Keep lint ranges inside the document while editing.
- [9667126](https://github.com/jspreddy/dql/commit/966712658222d02e1c61522d96f36710444dd76b) Lint the open file on save, fetch, and open.

## dqlrs 0.7.2-dev0 (2026-11-07)

Contributors: [Sai Jonnala](https://github.com/jspreddy)\
Agents: Cursor Agent

- [6835360](https://github.com/jspreddy/dql/commit/68353600ef89c21e7777cac024ed75ce85f03202) update some example table names for clarity ([#130](https://github.com/jspreddy/dql/pull/130))
- [7a6497f](https://github.com/jspreddy/dql/commit/7a6497f3275a17103a749ce0d7b1f6830c294b35) moving around local and temp table examples
- [75b7790](https://github.com/jspreddy/dql/commit/75b779031abc4cf05b47f886439331df9e34c86d) Name the tables in the SSE tables event.
- [798cd2e](https://github.com/jspreddy/dql/commit/798cd2ee6e01219673ef543d1eda52663a9f04f7) Add example queries for local tables and temp tables.
- [30e6ca2](https://github.com/jspreddy/dql/commit/30e6ca2eeaf32c7f8a950d694d3b7fd50f0e3d3e) Test that the editor caret flips between black and white.
- [6803c7f](https://github.com/jspreddy/dql/commit/6803c7fa0837a021174b7cf90e0dcc476807646b) Show a white editor cursor in dark mode.
- [4de3c20](https://github.com/jspreddy/dql/commit/4de3c20f718dad195b4c72bad41108c130d775dd) Cover the remaining web API routes with HTTP tests.
- [dfccec5](https://github.com/jspreddy/dql/commit/dfccec566efe2d66f3fd81b360708564ce1ff5dc) Push file, tree, presence, and table changes on one event stream.
- [f9e203b](https://github.com/jspreddy/dql/commit/f9e203bb2760b261d79dd0f2e00cb156d556dd62) Keep a dirty editor when a file poll sees a newer copy.
- [3a43aeb](https://github.com/jspreddy/dql/commit/3a43aeb38451dcbf81da5b2477196b549cdae637) format
- [7b823cf](https://github.com/jspreddy/dql/commit/7b823cff1e2ccf357b4c3c131cc55366eecca87d) Refactor file deletion logic and update tests for improved clarity. Adjusted confirmation messages for folder deletion and ensured proper handling of non-empty folders in both the frontend and backend. Updated Playwright tests to reflect these changes and verify correct error messages.
- [160bb1e](https://github.com/jspreddy/dql/commit/160bb1e7865eebade3747574df57cbbce5d0a666) Describe result structure in Rust.
- [5e4f5d0](https://github.com/jspreddy/dql/commit/5e4f5d0ac871cf5f4115872acecebf4343db4737) Show result structure for a data file page.
- [c7d13ef](https://github.com/jspreddy/dql/commit/c7d13ef737d16578c0f571df4d5fd29c76296de4) Open CSV and JSON files as paged tables.
- [488ce2f](https://github.com/jspreddy/dql/commit/488ce2fd7365629129ddd104da184b292e632285) Show CSV and JSON files in the web file tree.
- [3416246](https://github.com/jspreddy/dql/commit/341624643c1ea77d13228ab21cf017d9635fd9c3) Add local file tables and process-local temp tables.
- [5cf42fa](https://github.com/jspreddy/dql/commit/5cf42faedecd774f97e9df97bace859064e13ab1) Tell agents to use Playwright for the web UI. ([#131](https://github.com/jspreddy/dql/pull/131))
- [faeed7d](https://github.com/jspreddy/dql/commit/faeed7d8f8ba4964c727977ba9ce859e48de8927) Save new Playwright screenshots with playwrightArtifact ([#121](https://github.com/jspreddy/dql/pull/121))
- [bb9fe59](https://github.com/jspreddy/dql/commit/bb9fe59b02ce54a4dbb74cccd516413c9212ea64) Seat the editor filename underline on the header edge
- [fc11e7a](https://github.com/jspreddy/dql/commit/fc11e7a7f912e28664d222356726bf6c80b49cbe) Underline the open filename in the editor
- [df3b45a](https://github.com/jspreddy/dql/commit/df3b45a70c208c9f3f2fcc3aeefbdd7246667c61) Mark the open file and the file that is running
- [4383b83](https://github.com/jspreddy/dql/commit/4383b83b069d7ecddde98ae5fa8c4ccd35f090c1) Add Run file to the file context menu
- [abc93e6](https://github.com/jspreddy/dql/commit/abc93e6e162ed36d3baebd679454231b8824f0d3) Keep the API and MCP token in the user key store ([#120](https://github.com/jspreddy/dql/pull/120))
- [641de69](https://github.com/jspreddy/dql/commit/641de6914e5f26a987203109882785f75b838cb4) just recipe for dd-clean ([#119](https://github.com/jspreddy/dql/pull/119))
- [458dacf](https://github.com/jspreddy/dql/commit/458dacf0198e9343af052f9caf9589819aca4341) Group the DynamoDB Local recipes and alias dd to dynamo.
- [ebcfcb0](https://github.com/jspreddy/dql/commit/ebcfcb075765d69ce54c8c817263ba950afb3a3f) Add just dd-clean for DynamoDB Local data.

## dqlrs 0.7.1-dev0 (2026-10-30)

Contributors: [Sai Jonnala](https://github.com/jspreddy)\
Agents: Cursor Agent

- [1e3cd92](https://github.com/jspreddy/dql/commit/1e3cd92fd71eba0167e3edbbf60e8140d57f4e58) Add support for handling missing login keychain in key management ([#113](https://github.com/jspreddy/dql/pull/113))
- [c5833df](https://github.com/jspreddy/dql/commit/c5833dff1e51eba3dddacb68b1be0d0abe26f947) Zeroize transient cache key buffers on drop.
- [5c6a30e](https://github.com/jspreddy/dql/commit/5c6a30ea19f90167c219d805a3a628e659d73cd0) Seal both disk caches with AES-256-GCM.
- [f36607e](https://github.com/jspreddy/dql/commit/f36607ee1e0e6524501581d9885945f51adf92b0) Require a per-process token on the local API. ([#112](https://github.com/jspreddy/dql/pull/112))
- [ee7fd1d](https://github.com/jspreddy/dql/commit/ee7fd1ddba887bc811929531bbc2c9a45270f751) Plan encryption for the workspace result cache and the local API. ([#111](https://github.com/jspreddy/dql/pull/111))
- [09a5b8f](https://github.com/jspreddy/dql/commit/09a5b8fb5c2bde5efcaefab7727416e551dce4f4) Run just fix and just check before committing. ([#110](https://github.com/jspreddy/dql/pull/110))
- [4b8cd18](https://github.com/jspreddy/dql/commit/4b8cd184c3f9aa67307d51179295836619471754) Make just check report-only and add just fix.
- [e126347](https://github.com/jspreddy/dql/commit/e126347b3af7f5743fe998758b9dd8a7fa3e9083) Keep statement drags on the line ends. ([#109](https://github.com/jspreddy/dql/pull/109))
- [ff75f1c](https://github.com/jspreddy/dql/commit/ff75f1c93821606f3fa880c05bad1c32d3084f36) Drag editor selections in steps so the outline keeps its range.
- [66823a1](https://github.com/jspreddy/dql/commit/66823a15c051716389bdf3ad22e7ec1a039a6029) Say when just verify succeeded, and describe the recipe that failed. ([#108](https://github.com/jspreddy/dql/pull/108))
- [72b82ac](https://github.com/jspreddy/dql/commit/72b82ace91bc735c5ac3a84975628e6805793e50) Add --load-tables to warm the table cache when the web server starts. ([#107](https://github.com/jspreddy/dql/pull/107))
- [6e3dcfc](https://github.com/jspreddy/dql/commit/6e3dcfc53228c4522fc25390fb15fd8b86d83676) Add Ctrl+\ as another way to run the selection. ([#106](https://github.com/jspreddy/dql/pull/106))
- [8c6660d](https://github.com/jspreddy/dql/commit/8c6660dd37ab9f8ee54d83540c201b6b42ca4da0) rules
- [a4aa3e5](https://github.com/jspreddy/dql/commit/a4aa3e5a7944a622428b751d0b43553572d65e40) guard rails specificity
- [4db98d6](https://github.com/jspreddy/dql/commit/4db98d63211c33b29e644f387f29b36bd70d3173) adding hooks and other guard rails for the examples directory
- [f2f294a](https://github.com/jspreddy/dql/commit/f2f294a745dc60bdde3b2013836e8fac9536bb77) Save file-watch screenshots under .playwright-artifacts. ([#103](https://github.com/jspreddy/dql/pull/103))
- [7cc9ea2](https://github.com/jspreddy/dql/commit/7cc9ea21310f54c1df67918e95bdf05b11a60f12) Show the same connection summary when any profile icon is hovered.
- [e9753b4](https://github.com/jspreddy/dql/commit/e9753b46b5f4aaea2f858caca6a6b77ad5eb825c) Shift the identity block's slanted edge left by 10px.
- [cb716d8](https://github.com/jspreddy/dql/commit/cb716d8e8938e31303c0130f05272c51bba85185) Angle the identity block so its right edge is a forward slash.
- [e218673](https://github.com/jspreddy/dql/commit/e21867363a28bb54b0709e65431bd62254b4fc4e) Put this window's identity on the logo instead of the whole title bar.
- [4eaa7cd](https://github.com/jspreddy/dql/commit/4eaa7cdb45cacdda6b107cdb058d268f3ecaa36b) Name each open tab dqlrs: W1 and paint the title bar with that color.
- [cf2f5e5](https://github.com/jspreddy/dql/commit/cf2f5e5a59dfceed10e7c82a57d50f42315c2170) Refresh query files that change on disk and show who is connected.
- [cb70f0e](https://github.com/jspreddy/dql/commit/cb70f0e33bcdbb81b9e1f8f6536c2fe2a715c44c) Add a just-verify skill that prints headings and the exit code. ([#105](https://github.com/jspreddy/dql/pull/105))
- [2ba2a36](https://github.com/jspreddy/dql/commit/2ba2a3687fa898af69f54500d1c1bea3134bf33f) Tell MCP clients when to edit .dql files locally. ([#104](https://github.com/jspreddy/dql/pull/104))
- [ccb081e](https://github.com/jspreddy/dql/commit/ccb081e45479d25c10aba53f569c85f4d64af733) Add an MCP help tool for DQL syntax.
- [766785b](https://github.com/jspreddy/dql/commit/766785b008f9645bc9c2694677d85eb3d4563e40) Refresh the day-binned work metrics page. ([#102](https://github.com/jspreddy/dql/pull/102))
- [f4e48d6](https://github.com/jspreddy/dql/commit/f4e48d69d01dde25de490fb9ccfa82fa39c98631) Show local and global indexes as tables on the describe card ([#101](https://github.com/jspreddy/dql/pull/101))
- [7494928](https://github.com/jspreddy/dql/commit/7494928582ba57730647fd66cd9a1a70cfec7869) Drop a needless borrow in the formatted schema test ([#100](https://github.com/jspreddy/dql/pull/100))
- [22a6532](https://github.com/jspreddy/dql/commit/22a65326b1d611c406554fef1b6516ca9d9ee040) Keep the row view button at the left edge while a table scrolls sideways ([#99](https://github.com/jspreddy/dql/pull/99))
- [2debcd2](https://github.com/jspreddy/dql/commit/2debcd2ce2eeac836192ed3e90b00031edc74403) Format the CREATE TABLE statement shown for a described table ([#98](https://github.com/jspreddy/dql/pull/98))
- [f6448f2](https://github.com/jspreddy/dql/commit/f6448f2dd31a531633d1efa5754928539a3cde75) Keep every table column header fixed while rows scroll ([#97](https://github.com/jspreddy/dql/pull/97))
- [7c046a6](https://github.com/jspreddy/dql/commit/7c046a695c9d56123e924a116fb399e1fdc2eed2) Match table patterns against the full table list ([#96](https://github.com/jspreddy/dql/pull/96))
- [80f50b8](https://github.com/jspreddy/dql/commit/80f50b8c487f0b151340f33d40cb4d973e47516a) skill + data
- [e943d0d](https://github.com/jspreddy/dql/commit/e943d0d6737f8f0bea21af90826fbdd6e2ecb5a5) List category descriptions under Areas of work. ([#84](https://github.com/jspreddy/dql/pull/84))
- [5724079](https://github.com/jspreddy/dql/commit/57240792119de95eb29d348d7b7232c200155658) Add a pie chart of hours worked by category.
- [9978536](https://github.com/jspreddy/dql/commit/9978536f2e9bc9ea79e512cd4a0ea3a01b420ca1) Draw timeline tokens as green dots.
- [d65c5aa](https://github.com/jspreddy/dql/commit/d65c5aaa68220fff7671d7beb0ba2b0f55cc6cc8) Line commit circles up above the hours bars.
- [49efe3a](https://github.com/jspreddy/dql/commit/49efe3aa28de90ab9f47162a868485fb18c5f6fe) Count worked hours from commits and report them by day.
- [60631eb](https://github.com/jspreddy/dql/commit/60631ebc9d2518cb5d3419cc3dc5289ad7217eb8) Publish the work-metrics page on an hour bin.
- [5afd049](https://github.com/jspreddy/dql/commit/5afd04913f56c5a6390fd78670c0f3ae3aea2935) Separate chart series with a wider hue palette.
- [b4a6441](https://github.com/jspreddy/dql/commit/b4a64410646f9f946fe761b41220a158905f6470) Keep this repo's agent usage even in hours without a commit.
- [7b603e8](https://github.com/jspreddy/dql/commit/7b603e8c13049737c64808653c66d75456aab3e9) Add a work-metrics skill that charts commits, hours, and token spend.
- [4ffeb6b](https://github.com/jspreddy/dql/commit/4ffeb6b859d367caf2f9faa7a2e53b07eed775b3) Draw each day's commits as a circle sized by the count.
- [d296c3d](https://github.com/jspreddy/dql/commit/d296c3d6ebdc8f11b50921f884dc1a0fde2ac386) Plot commits and tokens on the daily hours graph.
- [bcc5c45](https://github.com/jspreddy/dql/commit/bcc5c45ec1190a98a6a607a88024c19db8886e38) Plot hours worked per day and split out Rust improvements.
- [ef0a1fe](https://github.com/jspreddy/dql/commit/ef0a1fe6816430891898193b62c327cef0c9b0c5) Add an hourly bin of the rewrite metrics.
- [e098c44](https://github.com/jspreddy/dql/commit/e098c44b34ede234599b994372f15e97da76608f) Add days worked for each rewrite feature group.
- [9c571ef](https://github.com/jspreddy/dql/commit/9c571ef3daa789bd51e1aae4e8b40866e3b98d91) Add a rewrite metrics page to the Rust docs.
- [6dc02db](https://github.com/jspreddy/dql/commit/6dc02dbce03f02a5b562d32b763d7092f198cdf1) Drop the extra columns table ported from Python. ([#80](https://github.com/jspreddy/dql/pull/80))

## dqlrs 0.7.0-dev0 (2026-10-22)

Contributors: [Sai Jonnala](https://github.com/jspreddy)\
Agents: Cursor Agent

- [93074c5](https://github.com/jspreddy/dql/commit/93074c56873a6589d046b8e799d2094dd1019129) cleanup ([#95](https://github.com/jspreddy/dql/pull/95))
- [e8256d2](https://github.com/jspreddy/dql/commit/e8256d2bde714bdd374034e12a5e8a04f961e48a) Add Playwright tests for nested attribute examples in SELECT and SCAN queries
- [4d4af7e](https://github.com/jspreddy/dql/commit/4d4af7e6c4479cd6cfdec6276903ec57b6a59623) examples for dot notation in queries
- [bcfd5b4](https://github.com/jspreddy/dql/commit/bcfd5b4f7ed262b042e3d47616f3191ed6a76f8e) examples: nested attributes
- [5359bd8](https://github.com/jspreddy/dql/commit/5359bd8fd4bff244e425c47535b19e91ffca31ab) Refactor run button UI and tooltip behavior in editor tests and application.
- [525fa35](https://github.com/jspreddy/dql/commit/525fa35a5e32fc16b4d47c62a84e90f75a105cee) Enhance explain plan display in editor tests and UI.
- [ec9d44f](https://github.com/jspreddy/dql/commit/ec9d44fb5dea9726735bdd9352bbfd51c795f61d) Show real read capacity on result cards.
- [0603e17](https://github.com/jspreddy/dql/commit/0603e1787748efa237e4f31893932f2b0e9cf412) Scroll wide result tables sideways inside the card. ([#94](https://github.com/jspreddy/dql/pull/94))
- [9d5197a](https://github.com/jspreddy/dql/commit/9d5197ab562a4a2c7d4631f25d160f54d1d0ce8a) Clear folder query results from the file menu. ([#93](https://github.com/jspreddy/dql/pull/93))
- [10cc1fb](https://github.com/jspreddy/dql/commit/10cc1fb7e32a3cd65b09074c99269aeda28698a1) Order explain card fields in the pretty view and JSON. ([#92](https://github.com/jspreddy/dql/pull/92))
- [f523c62](https://github.com/jspreddy/dql/commit/f523c62fb6d4abc0c70cf12a0e89d18e6eb99662) Satisfy clippy on nested paths and LS globs. ([#91](https://github.com/jspreddy/dql/pull/91))
- [58d8f26](https://github.com/jspreddy/dql/commit/58d8f26ceedc0f84c42f81730f6a5d41cea41085) Select nested map and list paths and alias columns.
- [a4b2fe5](https://github.com/jspreddy/dql/commit/a4b2fe5b4fc77590d8b86823949df2709a07e50e) Make LS a statement instead of a meta command. ([#90](https://github.com/jspreddy/dql/pull/90))
- [c880473](https://github.com/jspreddy/dql/commit/c880473d72fe9e268aeb29a5335f88c39ae60132) Add Result, History, and Lint tabs to the result pane. ([#89](https://github.com/jspreddy/dql/pull/89))
- [9461b74](https://github.com/jspreddy/dql/commit/9461b74c03249030d85a0732f58444b438947f6b) no query section on help result cards ([#88](https://github.com/jspreddy/dql/pull/88))
- [020efa7](https://github.com/jspreddy/dql/commit/020efa718e11fb86e481096800c62ad4b870bba8) minor visual updates
- [80a6816](https://github.com/jspreddy/dql/commit/80a6816221a3f803bd441335da22740a644f1c61) title bar layout
- [f5d2013](https://github.com/jspreddy/dql/commit/f5d201321786645690bcc26bb0813ca29b0e5c2b) Run mcp-test from verify.
- [6df0684](https://github.com/jspreddy/dql/commit/6df06847ff99af20fe3e2d4ec6104d2007f101dd) Add an mcp-smoke recipe and run it from verify.
- [bd83f52](https://github.com/jspreddy/dql/commit/bd83f52ac0a7c25b71da018a8f2d00a0df4c5a69) Ignore the ls cache hint in the DESCRIBE comparison.
- [40192ff](https://github.com/jspreddy/dql/commit/40192ff05a2a718a6310b6a6a5122dd2d8138b10) Write Playwright screenshots under .playwright-artifacts.
- [8b01e21](https://github.com/jspreddy/dql/commit/8b01e21d9b48b2f306667cb5270e20abf3b31473) Show the ls cache hint as an icon on the result card.
- [d58a513](https://github.com/jspreddy/dql/commit/d58a513543a06f11acd8b7651bb47cd89bd48e3b) Reload SHOW TABLES and DESCRIBE from the database.
- [bc43bd4](https://github.com/jspreddy/dql/commit/bc43bd46393939862241e2876c6897166146d961) Skip the ls cache hint when building the web table card.
- [c9270b0](https://github.com/jspreddy/dql/commit/c9270b0cd5fd0f6d5cd4c714bad951f469f25c92) Print attribute names and values in EXPLAIN plans.
- [09f932d](https://github.com/jspreddy/dql/commit/09f932d6e1d1fd9d7bbe4376b30f3926a958c3a3) Make HELP a query with a highlighted page. ([#87](https://github.com/jspreddy/dql/pull/87))
- [0ed8935](https://github.com/jspreddy/dql/commit/0ed893521e1ee380cfb8dc703ca4a8111a96eab7) Make help examples runnable on a fresh session.
- [c03e572](https://github.com/jspreddy/dql/commit/c03e57234ce00d8fb4574eff3c5edfe31bc2384c) Prove that lint does not run the script it checks. ([#86](https://github.com/jspreddy/dql/pull/86))
- [ad71167](https://github.com/jspreddy/dql/commit/ad71167a61b4821fd3ce92e8de9022ed731fa4f8) Add an MCP tool that lints a workspace query file.
- [1cbcba4](https://github.com/jspreddy/dql/commit/1cbcba45e016f50c3d4a2fd98b82ec767569b681) Add query linting in the web editor and as an MCP tool.
- [acd16eb](https://github.com/jspreddy/dql/commit/acd16ebaad1d9c8eb0bc4b37fbe3468ea699f66b) Print MCP setup hints when the web or MCP server starts. ([#83](https://github.com/jspreddy/dql/pull/83))
- [d775c78](https://github.com/jspreddy/dql/commit/d775c78c008933b6c66daa1e71d5b0adcbb28781) Serve MCP at /mcp when the web server starts with --mcp.
- [5d95e63](https://github.com/jspreddy/dql/commit/5d95e6364153b9d8ad9939618cd27fd8e26f5757) Add a dqlrs MCP server for the web workspace.
- [f70929d](https://github.com/jspreddy/dql/commit/f70929dd2665cf59ba07d3b520c8d10a5fc5c40f) Add a title-bar search for tables, columns, and queries. ([#82](https://github.com/jspreddy/dql/pull/82))
- [f8b6d98](https://github.com/jspreddy/dql/commit/f8b6d98f980ed8de940daec271e9b104d4543394) Show the table cache age in the Settings menu. ([#81](https://github.com/jspreddy/dql/pull/81))
- [124c439](https://github.com/jspreddy/dql/commit/124c4398ae52e8c6aaf9252ed8d5c6d70f5edc0f) Add a Settings menu with highlight options and clear table cache.
- [5731b13](https://github.com/jspreddy/dql/commit/5731b137cdc1d2ba2613a97ed1e7cf5064239428) Show the table cache age beside the refresh button.
- [795c67a](https://github.com/jspreddy/dql/commit/795c67a452ad622811b0441a11380c2fd5bc2acb) Cache table lists and definitions as they are fetched.
- [d99bf58](https://github.com/jspreddy/dql/commit/d99bf58aeaf3711bbb865b727ecb4dbc527983ba) Show the full structure sample and scroll sideways. ([#79](https://github.com/jspreddy/dql/pull/79))
- [c6bfa45](https://github.com/jspreddy/dql/commit/c6bfa45d736763383c2802ae77849beebd8623ee) Size structure columns to their content and give Sample the rest. ([#78](https://github.com/jspreddy/dql/pull/78))
- [c081451](https://github.com/jspreddy/dql/commit/c081451bc0ed96d64060936ae54ad73d4b1f3ed2) Fill the screen with the result structure and row popups. ([#77](https://github.com/jspreddy/dql/pull/77))
- [41f3630](https://github.com/jspreddy/dql/commit/41f36308f0673637b053e4d85b079ce22a80cdea) Show a retry when the web server goes away. ([#76](https://github.com/jspreddy/dql/pull/76))
- [46bbb5c](https://github.com/jspreddy/dql/commit/46bbb5c20326b596d994fcb452fa8a7afe4c467f) Fill the screen with the table structure popup. ([#75](https://github.com/jspreddy/dql/pull/75))
- [7dfef4e](https://github.com/jspreddy/dql/commit/7dfef4e492cdd79f056a0d3a57bd89416bc85c14) fix flakey test ([#74](https://github.com/jspreddy/dql/pull/74))
- [fbecdf6](https://github.com/jspreddy/dql/commit/fbecdf675d3c84235b847c7677a71798291a5351) Run the git graph script first and keep install steps in the FAQ.
- [4d20f3e](https://github.com/jspreddy/dql/commit/4d20f3e1b34fcdc1efdb9813acfe9294d5a3d6fe) Limit the git graph to open branches from the last two days.
- [f4d76b0](https://github.com/jspreddy/dql/commit/f4d76b05df155be09ba478a32bbf86b096248460) Drop git graph commits older than one week before origin/v-rust.
- [98c36bc](https://github.com/jspreddy/dql/commit/98c36bc8808de47ac45707ff46e7a6075fb47d3c) Draw the git graph from origin branches after fetch.
- [2ad9178](https://github.com/jspreddy/dql/commit/2ad91786faa640f21075b04aa0ca077acc8efae3) Limit the git graph to commits that are not already in v-rust.
- [30bcdb9](https://github.com/jspreddy/dql/commit/30bcdb9bbf86c230b5701c00d36f73368ca61b7b) Store web query results in .dqlrs-web-cache and clear them from the nav menu. ([#73](https://github.com/jspreddy/dql/pull/73))
- [d5be5eb](https://github.com/jspreddy/dql/commit/d5be5ebddea8a758468dd467716ce1459242ef77) Suggest catalog tables and keys in the editor. ([#72](https://github.com/jspreddy/dql/pull/72))
- [d25c8de](https://github.com/jspreddy/dql/commit/d25c8de82418554d8be49328fdd7ffac3c7dfaca) Show consumed capacity and elapsed time on the result card. ([#71](https://github.com/jspreddy/dql/pull/71))
- [3ddf318](https://github.com/jspreddy/dql/commit/3ddf3182da615e8cbb609acc8da679c8aced023f) Add a saved light and dark theme to the web UI. ([#70](https://github.com/jspreddy/dql/pull/70))
- [ec3b408](https://github.com/jspreddy/dql/commit/ec3b40804de7cadd339d1886f77e0845c1ab456d) Place the result card row count next to the structure icon. ([#69](https://github.com/jspreddy/dql/pull/69))
- [da28b30](https://github.com/jspreddy/dql/commit/da28b30465644e2ffc578079ad9331894820709a) Remove the query row-count chip from the Results bar.
- [75fcb37](https://github.com/jspreddy/dql/commit/75fcb370177475bd0a3cc8c50b17dae8963e4cfe) Show the AWS session in the header and open caller identity.
- [ecbdcb9](https://github.com/jspreddy/dql/commit/ecbdcb9989cbd6d3cc556fabf99adc0b1dd17770) Show the result count and clear icon in a right-aligned chip. ([#68](https://github.com/jspreddy/dql/pull/68))
- [161f977](https://github.com/jspreddy/dql/commit/161f977ff771d059247c3929816c6989594652f0) Pin the latest query result to the top of the results pane.
- [18d8f80](https://github.com/jspreddy/dql/commit/18d8f8037e091c76bfc85a3c4388fa349d7c7bfd) Match the results resize handle to the nav grip.
- [74396e3](https://github.com/jspreddy/dql/commit/74396e39ff8fd21f454ca0639320ffade5ed7602) Align the result query dot with the first line of text.
- [cac8fb2](https://github.com/jspreddy/dql/commit/cac8fb2b9b01435c686c5bfb233465876935c277) TestFix: Update CSS color assertion in editor tests to handle detached nodes correctly.
- [274ab6b](https://github.com/jspreddy/dql/commit/274ab6bb1340760a62d206956842b292cf419a47) Show a query dot on every result and hold its open size.
- [150b13d](https://github.com/jspreddy/dql/commit/150b13d74f284221d2b347d8d1fa2bbbed710f71) Replace the result-card chevron with a quiet dot.
- [1c0c270](https://github.com/jspreddy/dql/commit/1c0c2704dda8e5a50aee71d0f3683a8dde5f097f) Pin a smaller result-card chevron to the top of the card.
- [3449e5c](https://github.com/jspreddy/dql/commit/3449e5c07315f693f08554febdfa334de4d32077) Show a result card's query from a left chevron.
- [7ed18bb](https://github.com/jspreddy/dql/commit/7ed18bb359d3737e266fc2d33a9797823e489c3b) Store each statement as its own query result file.
- [19fbf25](https://github.com/jspreddy/dql/commit/19fbf2538c2f11df4d0dfb619be4afc65c584e88) Stop echoing the query text above result cards.
- [4d8a2a6](https://github.com/jspreddy/dql/commit/4d8a2a6bfdf472cae538cb3e8e9819e92c823c80) gitignore .cache files
- [adf677d](https://github.com/jspreddy/dql/commit/adf677d539672fa9263376d42ddde56dea0ef22d) Keep web query results with each file.
- [8e28e8b](https://github.com/jspreddy/dql/commit/8e28e8b41abfbd36a9680a73505af0a293eaf77e) Color Run file and Run folder as danger actions. ([#67](https://github.com/jspreddy/dql/pull/67))
- [25d490b](https://github.com/jspreddy/dql/commit/25d490bc1217f55ca029c29b469be409a0b40029) Mark Run file and Run folder with a fast-forward icon.
- [7bb77e5](https://github.com/jspreddy/dql/commit/7bb77e50121cbb9f7a1fd5eaa20a8c37dc4c198d) Run a whole DQL file or folder from the web editor.
- [2ae6e71](https://github.com/jspreddy/dql/commit/2ae6e71d47fc667c33356fa01ce21a8d4b097fe1) Show the other key chips under the row popup title. ([#65](https://github.com/jspreddy/dql/pull/65))
- [f64b280](https://github.com/jspreddy/dql/commit/f64b2801f896b97d426fbdacc512849db3fddeb7) Add functionality to close row popover on outside click and update tests
- [4e5d0cd](https://github.com/jspreddy/dql/commit/4e5d0cdf1dd084ae0bb104269952d88934669035) Limit the git graph to commits that are not already in v-rust.
- [8ecc378](https://github.com/jspreddy/dql/commit/8ecc378b32c298b5aa6ba097d853effa906fc096) Drop to_string from a result-card width assertion.
- [c11f67d](https://github.com/jspreddy/dql/commit/c11f67df079e9003698fe4cb7b4ccb7a5d4d8245) Show a result row as highlighted JSON from an eye icon.
- [1eaa004](https://github.com/jspreddy/dql/commit/1eaa00401b49189a10e6c3f741108ea80e46c1e1) Scroll wide results with the left and right arrows. ([#64](https://github.com/jspreddy/dql/pull/64))
- [c4244da](https://github.com/jspreddy/dql/commit/c4244dafe71dcaa9a38046d1b4ff180064f11243) Draw the clipped-column note with the results card block. ([#63](https://github.com/jspreddy/dql/pull/63))
- [2fc7eb1](https://github.com/jspreddy/dql/commit/2fc7eb1d881eb23cfe7e7fd1aace946e5a4f5e6d) Show clipped columns in an info panel.
- [c5850c2](https://github.com/jspreddy/dql/commit/c5850c2ff6b2d22d7ae1003ab83d87ece349ea58) Name result columns the flush clips off screen.
- [98295b1](https://github.com/jspreddy/dql/commit/98295b1b701ba82d317f9ba6b68fc88257b077a6) Paint the CLI results card with the TUI palette.
- [5f93291](https://github.com/jspreddy/dql/commit/5f9329108be5280eb913af9b7856bd24f70c5587) Style the CLI results table like the web results card.
- [a82e285](https://github.com/jspreddy/dql/commit/a82e285d90a97f87f27cb236c5a56368464c48d3) Scroll clear so the REPL input returns to the top. ([#62](https://github.com/jspreddy/dql/pull/62))
- [1efb2ff](https://github.com/jspreddy/dql/commit/1efb2ff1b7fd18eb76c4f5eee2be20ba9315e2f7) Color the git graph HEAD label blue. ([#61](https://github.com/jspreddy/dql/pull/61))
- [8d006b4](https://github.com/jspreddy/dql/commit/8d006b4de96ce24d0a5cc6e10ef8ac99e948e21d) Drop the git graph skill's check instructions.
- [79c8ca0](https://github.com/jspreddy/dql/commit/79c8ca097f1afc701b1c6c00f36e795c149a5b12) Keep the git graph white background in the script.
- [3b1565f](https://github.com/jspreddy/dql/commit/3b1565f54185ec0823a1b68dc87d7c92d45ddfd2) Keep the git graph skill to a single SVG.
- [a807c47](https://github.com/jspreddy/dql/commit/a807c474abb581b27e309f6706fd329f014349d2) Paint the git graph SVG with an opaque white background.
- [aed199a](https://github.com/jspreddy/dql/commit/aed199a2ab8e5e079b205356ab85e58499dfea04) Give the git graph page a white html background.
- [14db3df](https://github.com/jspreddy/dql/commit/14db3df910c238c861d7d97afb46e255754c6c89) Add a git-graph skill for one screenshot above v-rust.
- [a8a1884](https://github.com/jspreddy/dql/commit/a8a18846eb48a5a5b94e203706db84e6cb5db747) Add an in-memory Ratatui test suite for the REPL. ([#60](https://github.com/jspreddy/dql/pull/60))
- [c217a13](https://github.com/jspreddy/dql/commit/c217a13346095d6be8c2d6f6e587c25e31d9558b) Require a semicolon to submit SQL in the REPL. ([#58](https://github.com/jspreddy/dql/pull/58))
- [e866ee9](https://github.com/jspreddy/dql/commit/e866ee9f579ba7984ca33efd6411e9b2287b747d) Use a black terminal background for CLI screenshots.
- [5f3bcd1](https://github.com/jspreddy/dql/commit/5f3bcd18659d5e20e59afd6ebfa4d0dc378323b5) Recapture CLI screenshots with rich color from a pasted SELECT.
- [74a4959](https://github.com/jspreddy/dql/commit/74a4959a98955aa5d90b1d04dd08a8898efc1dec) Fix REPL SQL submit after paste and semicolon stripping.
- [6707929](https://github.com/jspreddy/dql/commit/6707929c1a24e89d8b1cb53c6d13184f1ecf371b) Organize product screenshots by interface. ([#57](https://github.com/jspreddy/dql/pull/57))
- [9bc2e6e](https://github.com/jspreddy/dql/commit/9bc2e6e84d83277acd0f91238634f140dea330c4) Refresh the notebook README screenshot in a clean sample folder.
- [94058ee](https://github.com/jspreddy/dql/commit/94058eea5ea4112d341178803283dd44e1f20845) Move maintainer notes out of the README.
- [73fb1ed](https://github.com/jspreddy/dql/commit/73fb1edd438c9a3941ab4d3a6ff40506cb93cc84) Rewrite the top-level README for developers.
- [ab4e139](https://github.com/jspreddy/dql/commit/ab4e1391252ac0c0b063289c291bef7e04e61d12) Use cargo install --git for the end-user Rust install. ([#56](https://github.com/jspreddy/dql/pull/56))
- [31385f4](https://github.com/jspreddy/dql/commit/31385f4c9a94786498e87b09c56e13c2f9b86c9e) Document installs from the v-rust branch.
- [73be2bf](https://github.com/jspreddy/dql/commit/73be2bfcfaee3ef3bb5359bb59e7a41c0f0021da) just check
- [8b13a78](https://github.com/jspreddy/dql/commit/8b13a78d73eb1118d240ef121b409fa6782bedca) Reapply "Add a Stop control for a running web query."

## dqlrs 0.6.5-dev1 (2026-10-12)

Contributors: [Sai Jonnala](https://github.com/jspreddy)\
Agents: Cursor Agent

- [a66fe85](https://github.com/jspreddy/dql/commit/a66fe8586ffd0043fb37ad9a731854f6a9ac2122) Fix flakey test ([#54](https://github.com/jspreddy/dql/pull/54))
- [ae6b299](https://github.com/jspreddy/dql/commit/ae6b299e34749e848c81fcc898e81491118c9f24) just check fix
- [62da4a0](https://github.com/jspreddy/dql/commit/62da4a0b420e8048b051fc2d36cd8fca3227a285) just check
- [fe1ce54](https://github.com/jspreddy/dql/commit/fe1ce543df61f6423147bfd6686faa504a4531e0) Add condition-function queries to py/rs parity tests.
- [c2e982c](https://github.com/jspreddy/dql/commit/c2e982ce9c57de3b5dd41cba627dd466e85b36ff) Add sample queries and Playwright coverage for DynamoDB functions.
- [3a140dd](https://github.com/jspreddy/dql/commit/3a140dd4ae128e1669c1f1ade220c044df231733) Keep contains() filters when a projection is present.
- [88c74d5](https://github.com/jspreddy/dql/commit/88c74d51e9ca1958769012b195a13264e4de8aba) Keep the info control inside the Result structure kicker. ([#53](https://github.com/jspreddy/dql/pull/53))
- [04032eb](https://github.com/jspreddy/dql/commit/04032ebfafe38967eee5a465972a7478dffec9fb) Keep the structure info icon on the Result structure line.
- [2bac523](https://github.com/jspreddy/dql/commit/2bac52383b66168ec08a9173208c599bd983568e) Restack the result structure popup title.
- [c23e826](https://github.com/jspreddy/dql/commit/c23e826611cd7d5c9c5b23bff9c0ff284ab56991) Move the result query into the structure popup title.
- [63ffcda](https://github.com/jspreddy/dql/commit/63ffcda6f3ccbb5e3d237ea2c26932af646307e8) Show the ran query in the result structure popup.
- [7b545e7](https://github.com/jspreddy/dql/commit/7b545e74e54b5c80618df50529084db775f7b841) Put the structure icon on each query result card.
- [76e3f3c](https://github.com/jspreddy/dql/commit/76e3f3c1def1a07835edf88235682b9d2452f0c4) Use SCAN in the structure popup test.
- [9378539](https://github.com/jspreddy/dql/commit/9378539478fc552041654b4d2b1165a802d47287) Show result and table structure from data already in hand.
- [57497fe](https://github.com/jspreddy/dql/commit/57497fefae6670cd4806a203de501006e1f77dc9) Revert "Add a Stop control for a running web query." ([#49](https://github.com/jspreddy/dql/pull/49))
- [8adc8a5](https://github.com/jspreddy/dql/commit/8adc8a5624b2863a05533266c25757190585ccb1) Add a Stop control for a running web query.
- [6c0b469](https://github.com/jspreddy/dql/commit/6c0b469990b158032a7c5ccf155441f05ca53706) Keep the tables search on a cached catalog.
- [7000e58](https://github.com/jspreddy/dql/commit/7000e58931fb32ad37de8b1be4ca916544173f16) Let the left nav drag to resize and remember its width.
- [ecd5f26](https://github.com/jspreddy/dql/commit/ecd5f26327ed06b76a6d31fccf84950ff2d7f813) explainer
- [db2c948](https://github.com/jspreddy/dql/commit/db2c9487a5d451ba92ab3811f32f2dc7741a88c3) html-explainer skill
- [5b47bea](https://github.com/jspreddy/dql/commit/5b47bea4fda2eaa44799805a82f02eec282386ac) mermaid diagram skill
- [b2c1f5b](https://github.com/jspreddy/dql/commit/b2c1f5b244f19d481b2a8f9d1330d6af723f14cf) Add titles to Justfile recipes and create justfile.html documentation
- [f7127ac](https://github.com/jspreddy/dql/commit/f7127acf87ee65ca59658aff6c2a9d86af62a8ef) fix flakey test
- [62d8e62](https://github.com/jspreddy/dql/commit/62d8e626b1c3e92d80b63e02aa366feb29f5df81) Stop black-box flakes after create and on short name tokens.
- [d15db41](https://github.com/jspreddy/dql/commit/d15db415209b332e7bad1150910c158b1403ccf5) Let SAVE create missing parent directories.
- [353c691](https://github.com/jspreddy/dql/commit/353c6914508c64332fb330a1de6760a7a132ef88) Check SAVE paths before reading.
- [19b2693](https://github.com/jspreddy/dql/commit/19b269331acd914e3b64c11ba9360f2b2772e7db) Add Playwright coverage for SAVE and LOAD.
- [30893c0](https://github.com/jspreddy/dql/commit/30893c0fe164ac8aa731c750b3d6b8337038a46a) Add a SAVE and LOAD example query.
- [85e0d46](https://github.com/jspreddy/dql/commit/85e0d468fba02d9964f4c64826493e794e388ffa) remove old html file ([#48](https://github.com/jspreddy/dql/pull/48))
- [1385155](https://github.com/jspreddy/dql/commit/138515568bbeb1e2d382f074fe9d6292a8ed948f) Document how the web UI is packaged into dqlrs.
- [e435d2e](https://github.com/jspreddy/dql/commit/e435d2ec21bf60e188dd06e155df82522c68016c) commit rule
- [d92f835](https://github.com/jspreddy/dql/commit/d92f835312306aa8bbef5ce0dac7217721a10bc2) additional just recipes
- [27be742](https://github.com/jspreddy/dql/commit/27be742b34fa677631bd3c5ba1b4da8efcd26497) move recipes around
- [421265f](https://github.com/jspreddy/dql/commit/421265f2939b67a18a2fc08521bdf01d139409f7) Add web unit tests and enhance Playwright test configurations.
- [f8abd8f](https://github.com/jspreddy/dql/commit/f8abd8fa54f42d71d2b0adeba6c33a333fd0a5f9) Add architecture, package, and runtime diagrams for dqlrs.
- [985d2ba](https://github.com/jspreddy/dql/commit/985d2baecee50a98b2b1b2991e7000b49efd09c6) Use an empty SHOW TABLES pattern that does not match zzqq keys.
- [263e1ff](https://github.com/jspreddy/dql/commit/263e1ff1db5c6f5345993c65dc6929a5c3162542) Answer each web request before reading the next.
- [ddf7b84](https://github.com/jspreddy/dql/commit/ddf7b8402cf2167feb91113ca3b7b86b05e34d01) Build the web recipes with the rust-impl toolchain.
- [1dc09c0](https://github.com/jspreddy/dql/commit/1dc09c0a8ba9ef0bd25870288629fdea2712fee8) Serve the web UI from dqlrs --web.
- [99a8530](https://github.com/jspreddy/dql/commit/99a85305f4a4e345608c21a0ead374cbd6ca3751) Note that the target state is Rust.
- [bdb7199](https://github.com/jspreddy/dql/commit/bdb7199a592d7cd0f44a335dfbc56f0de45ec5d2) Query an index whose range is the table hash key. ([#47](https://github.com/jspreddy/dql/pull/47))
- [fc18365](https://github.com/jspreddy/dql/commit/fc183657a380b24d0a6c4f0760c58865d7fa66ba) Soften key column borders to match the grid.
- [b3a4a69](https://github.com/jspreddy/dql/commit/b3a4a6980185f57a3ebb5b4b68745f606da5ee3a) fix tests for cross platform runs ([#44](https://github.com/jspreddy/dql/pull/44))
- [159a479](https://github.com/jspreddy/dql/commit/159a479f9b4315ecad21ec497e6b1e1bf014c4e2) fix flakey test platform specificity
- [38cd05d](https://github.com/jspreddy/dql/commit/38cd05d60bf338a7297354a682601ba63308350e) Format ls and SHOW TABLES as the summary list.
- [52c66f1](https://github.com/jspreddy/dql/commit/52c66f11335cf4f4fbda64afdbd24105fcafbc2e) Shorten the tables search note so it fits the sidebar.
- [fb30851](https://github.com/jspreddy/dql/commit/fb3085112ad58ff7735aef2b5ed94212916427f4) Filter the tables sidebar with the same matching as ls.
- [41c47b8](https://github.com/jspreddy/dql/commit/41c47b8f2138cd1726203a647ddf062287b57e2f) Show missed SHOW TABLES LIKE results in the table list.
- [7749c4b](https://github.com/jspreddy/dql/commit/7749c4b422877e001402da1d875a13a6cc2749f8) Show similar names when SHOW TABLES LIKE matches nothing.
- [f124bf0](https://github.com/jspreddy/dql/commit/f124bf0465188e104de6f32fd8bb06d383e61ac6) Keep a partial name from matching every near-duplicate.
- [f260962](https://github.com/jspreddy/dql/commit/f26096280c1dc2e2ead3a04f776821e635e4023d) Rank missed ls patterns by similar names and related keys.
- [6368a96](https://github.com/jspreddy/dql/commit/6368a96f18e1fa7d6c4010a59fb3e952b11a3891) Keep the intelligent-match pattern highlight tight against the comma.
- [03c0af3](https://github.com/jspreddy/dql/commit/03c0af39c43857f7771272d210d3d8f0d8a3d05f) Beautify the intelligent-match result list.
- [120d991](https://github.com/jspreddy/dql/commit/120d9919ffd22b7461b4e4bb97b9cbd992d8399c) Show intelligent ls matches when nothing matches exactly.
- [113b525](https://github.com/jspreddy/dql/commit/113b525d7e4d24837234eb44817b57b9bb835a53) Beautify the table description card and highlight its query. ([#43](https://github.com/jspreddy/dql/pull/43))
- [8953ab2](https://github.com/jspreddy/dql/commit/8953ab21507c53cb04691d18822afd992a232012) Add DESCRIBE and print the ls description for one match.
- [aafea31](https://github.com/jspreddy/dql/commit/aafea317d87aca734a39dd4e0a1310731e7f77f2) Match both tables in the SHOW TABLES and ls glob acceptance tests. ([#42](https://github.com/jspreddy/dql/pull/42))
- [170e12d](https://github.com/jspreddy/dql/commit/170e12de921aba08124300e0f84fcec162e41892) Assert the web refresh test against throughput, which Local accepts.
- [13a9052](https://github.com/jspreddy/dql/commit/13a90523a857250e7e3e5387445780b61d6bf559) Add SHOW TABLES LIKE and honor ls glob refresh=True.
- [97e7ab6](https://github.com/jspreddy/dql/commit/97e7ab6199bcc77461da188dc4b251b9a50ed6bd) Add a refresh button that reloads the table list. ([#41](https://github.com/jspreddy/dql/pull/41))
- [788979e](https://github.com/jspreddy/dql/commit/788979e2c6c10485ab73bac8e7cbd36899e23338) Drop the duplicated editor imports that break the web UI. ([#40](https://github.com/jspreddy/dql/pull/40))
- [ca4a8d2](https://github.com/jspreddy/dql/commit/ca4a8d27509d8388ff56721b64ac8064a655f831) Paint text selection above the query bands. ([#37](https://github.com/jspreddy/dql/pull/37))
- [9447ac5](https://github.com/jspreddy/dql/commit/9447ac5e94e54eee5a88c8b0eb9b83f519568c52) Bring the floating run button back onto the editor.
- [879339e](https://github.com/jspreddy/dql/commit/879339e0cef680f603a0eb27bf056eb8e9671dcf) Separate text selection from the query window.
- [87eec16](https://github.com/jspreddy/dql/commit/87eec16cceba6b92f241b4dbf2592557a881aac9) Lighten key column cells and outline them. ([#36](https://github.com/jspreddy/dql/pull/36))

## dqlrs 0.6.5-dev0 (2026-10-10)

Contributors: [Sai Jonnala](https://github.com/jspreddy)\
Agents: Cursor Agent

- [0c43b1d](https://github.com/jspreddy/dql/commit/0c43b1db37b91a471116035e13c62d2951f20254) Add a floating play button beside the run selection. ([#35](https://github.com/jspreddy/dql/pull/35))
- [34c9d8e](https://github.com/jspreddy/dql/commit/34c9d8e69c7d4dc8c59c5278b21bf034711ea275) Show hash and range icons in the table list. ([#34](https://github.com/jspreddy/dql/pull/34))
- [e06ba1d](https://github.com/jspreddy/dql/commit/e06ba1d7978de7f897c8825105626062378e11d6) Mark hash and range key columns with icons and tint.
- [0b902b9](https://github.com/jspreddy/dql/commit/0b902b90dc7d2c30fb476241f3eded0a21e4ae3c) Order result columns by table keys, then index keys.
- [43906f6](https://github.com/jspreddy/dql/commit/43906f6b68667827f41704f1cc1af89eef750a44) Show the table list in the nav on Tables. ([#33](https://github.com/jspreddy/dql/pull/33))
- [ffddf6b](https://github.com/jspreddy/dql/commit/ffddf6b8c930758eb5ec49d0e77ba510565c2edb) Add a single-table cosmos sample. ([#32](https://github.com/jspreddy/dql/pull/32))
- [6041d35](https://github.com/jspreddy/dql/commit/6041d3559e7590b6f25d5c5e1bcec0b4842d1f48) Add sample queries with uneven item shapes.
- [3952862](https://github.com/jspreddy/dql/commit/3952862396cd059fe85ee2b37fdbddd0dfcd8039) Add a JSON toggle on the explain plan card. ([#31](https://github.com/jspreddy/dql/pull/31))
- [b084dd4](https://github.com/jspreddy/dql/commit/b084dd40ccc7da575aae413bcda1abe0bc23c877) Show explain plans as operations and conditions.
- [41384db](https://github.com/jspreddy/dql/commit/41384dbb81c3cb0e6656bd254d6d20936acbee78) Add just commands for the web app and its Playwright tests. ([#30](https://github.com/jspreddy/dql/pull/30))
- [35a5ccb](https://github.com/jspreddy/dql/commit/35a5ccbf0174b66211d18c7927c3984e6b8f1c6f) Make the results pane resizable and easier to scan. ([#29](https://github.com/jspreddy/dql/pull/29))
- [6e97228](https://github.com/jspreddy/dql/commit/6e972289245eee808486c1d0a89412470502682d) Count painted write bars in the highlight test. ([#28](https://github.com/jspreddy/dql/pull/28))
- [9792928](https://github.com/jspreddy/dql/commit/9792928d122eda92609d25ae660614b083352435) Add Playwright tests for the DQLRS web UI.
- [0de601a](https://github.com/jspreddy/dql/commit/0de601a7f6bd8ee6751eb0af83358978cb5c81f8) Add space above and below the editor lines. ([#27](https://github.com/jspreddy/dql/pull/27))
- [8837aae](https://github.com/jspreddy/dql/commit/8837aaeceaf34d08d2613fc6a64947a49f2c8617) Stop the cursor highlight at the end of the current query.
- [0cc1d86](https://github.com/jspreddy/dql/commit/0cc1d86e40f24e59516247d314fe7fc47a7497ad) Round the selection highlight box by 2px.
- [d1c69ba](https://github.com/jspreddy/dql/commit/d1c69ba6df03ffe60191712216c5227580af4872) Lower the selection wash to 5% opacity.
- [ed862c5](https://github.com/jspreddy/dql/commit/ed862c578c8919ae768b58b45a50abaafd123089) Pull the selection border in to 5px and tint the wash blue.
- [0c85827](https://github.com/jspreddy/dql/commit/0c858276cbef4f301fd3a621103ec65fd5ffe833) Lighten the run highlight to 10% and outline it in blue.
- [4774732](https://github.com/jspreddy/dql/commit/4774732051b9025696a052ec5eddcb560c59f939) Mark the run range with one background instead of a border.
- [5a2328d](https://github.com/jspreddy/dql/commit/5a2328d0ddb00e820588fc92027e4e9e68c316b0) Keep syntax colors under a 25% black text selection.
- [64be797](https://github.com/jspreddy/dql/commit/64be7973241c6d0552920560abd2d77121cf1b01) Show query progress beside Results and mark the query in the gutter.
- [8414bf7](https://github.com/jspreddy/dql/commit/8414bf7fbcf0849ac70902a92a8fe389f01108bb) Add an HTML explanation of what this branch adds.
- [00dd8a7](https://github.com/jspreddy/dql/commit/00dd8a7ff985b15993c0dc358794e6d78d162088) Manage files and folders from the query tree.
- [9b6c8ab](https://github.com/jspreddy/dql/commit/9b6c8ab0e468d035e22f6a701698068040b877b5) Draw the run border as a 1px black line at half opacity.
- [1fb2dfa](https://github.com/jspreddy/dql/commit/1fb2dfa7e403ce981f69ee3bda93049cbcac233b) Mark the query Run will execute and bind it to Ctrl+Enter.
- [6683cbe](https://github.com/jspreddy/dql/commit/6683cbe96cfab73e5bdf5f6d4d20b62e58d5f390) Add editor toggles for minimal write bars and even/odd bands.
- [0d2541b](https://github.com/jspreddy/dql/commit/0d2541b294942852ecf6fd872aecca7d7fac20c2) Add query and comment variations to the sample posts file.
- [cf928b9](https://github.com/jspreddy/dql/commit/cf928b9d3c6d455b3518e415e7e2d25caf7d514a) Include a file's final blank line in the previous query highlight.
- [f6bfce4](https://github.com/jspreddy/dql/commit/f6bfce45b1fedbc91d867bff0b74e3175c609595) Extend query highlights across touching comments and the following blank line.
- [6acb4b9](https://github.com/jspreddy/dql/commit/6acb4b92c19aee5707f57a0ae37d7b03ad553a3b) Darken the red band on even write queries.
- [fc56a08](https://github.com/jspreddy/dql/commit/fc56a0854430aa9f1f92417e21c5d44d76034f66) Note query bands and red DROP and DELETE in the web README.
- [f09ce7c](https://github.com/jspreddy/dql/commit/f09ce7c72ae78e256e5132bad907ff264e22a370) Tint alternate queries and mark writes in the editor.
- [29c8038](https://github.com/jspreddy/dql/commit/29c8038158a275303218c9a15235b296fbd1fb09) Highlight DQL queries with a CodeMirror mode.
- [cf2bba3](https://github.com/jspreddy/dql/commit/cf2bba3da3a0baf25d0f94e289531769343c6993) Implement the DQLRS Web query editor and table browser.
- [7c87f46](https://github.com/jspreddy/dql/commit/7c87f462ae8a01a284949289b7fc6f5ba1ddf6f3) Add denser DQLRS Web mockups with ls-style table search.
- [5bbd6ff](https://github.com/jspreddy/dql/commit/5bbd6ff58b619fab661337dce7f596496789d075) Add DQLRS Web interface mockups.
- [a151dfa](https://github.com/jspreddy/dql/commit/a151dfab6c2645d4937342d99ae15372be402fc9) explainer ([#25](https://github.com/jspreddy/dql/pull/25))
- [40f6b01](https://github.com/jspreddy/dql/commit/40f6b01565bbe1458efb01a0aea71f8ec6df310c) Add just dynamo-stop to choose which DynamoDB Local processes to stop.
- [1887257](https://github.com/jspreddy/dql/commit/1887257f81923628e7ad3e6101db1510e9792af1) Stop dqlrs notebook servers with one Ctrl+C or from another terminal.
- [2dcde2d](https://github.com/jspreddy/dql/commit/2dcde2d87ba23211785369583e6434f2c56f3590) just commands
- [1b39041](https://github.com/jspreddy/dql/commit/1b3904137cb7d7aa7a5c0b0e8cd264de634e5515) fix test for dynamic version
- [12c7612](https://github.com/jspreddy/dql/commit/12c7612ac665d5cbb6b68ea56f699a974f5bdcb9) Fix the TCP serve test hang on shared progress reads.
- [9e561c1](https://github.com/jspreddy/dql/commit/9e561c181a63698eb6ff21e741961f974cbe46d9) Add slow notebook test support and update test configurations.
- [0f966ee](https://github.com/jspreddy/dql/commit/0f966eea42bbc1fc8084ebc6718dfa138f8bb0e8) Fix Python lint and pin the Rich console size in tests.
- [06bdfbc](https://github.com/jspreddy/dql/commit/06bdfbc2d8d49eea961f06a444e67f7187f207cc) checkpoint
- [fc5f9fa](https://github.com/jspreddy/dql/commit/fc5f9fa945110177e0c21ceda9cc5465dad517c6) use "just" for top level dev entry point
- [e7ef37f](https://github.com/jspreddy/dql/commit/e7ef37f3556964324fa48d6154ab7802bd029ffe) Enhance notebook test runner with new options and documentation.
- [c7a80bd](https://github.com/jspreddy/dql/commit/c7a80bd88d67260babb44450585bdf5a362f7051) Run notebook Playwright tests in the installed Chrome.
- [e2a0c0a](https://github.com/jspreddy/dql/commit/e2a0c0a8b45d406c2de2346d79a14d8beb18b4f4) Add Playwright tests for the DQL notebook UI.
- [54397aa](https://github.com/jspreddy/dql/commit/54397aac0186580930b4057c4093055b5f9483cb) Stop SELECT/SCAN notebook progress bars after the query finishes. ([#22](https://github.com/jspreddy/dql/pull/22))
- [5955378](https://github.com/jspreddy/dql/commit/59553780f0ba15b19eb88b8934a2eb1bb32de478) Add black-box coverage for serve progress, notebook flags, and DROP IF EXISTS.
- [e420ec0](https://github.com/jspreddy/dql/commit/e420ec06f7d2df241127315ad420baa0b9ba0951) Treat DynamoDB ResourceNotFound as success for DROP IF EXISTS.
- [cdeb77e](https://github.com/jspreddy/dql/commit/cdeb77e60f8bec6002e3be0b54acfe356a5f5b2e) Document dqlrs notebook for cargo-installed binaries.
- [e3ac92f](https://github.com/jspreddy/dql/commit/e3ac92f44b40f714372eab1689fa9d973d8463ec) Add dqlrs notebook to launch JupyterLab from the installed binary.
- [f218f5b](https://github.com/jspreddy/dql/commit/f218f5bc67c9f21301faf3bb4f758c8e2766d2f5) Document serve progress events and notebook kernel attach.
- [0af1749](https://github.com/jspreddy/dql/commit/0af174965f85242ee095bb172c29583a4d250e4b) Show live bulk-write progress in DQL notebook kernels and magics.
- [47c91b0](https://github.com/jspreddy/dql/commit/47c91b003d4fb1cb3304aadd93d2d5975458e34b) Emit JSON and IPython progress from Python Engine bulk writes.
- [68b12d4](https://github.com/jspreddy/dql/commit/68b12d4998a4126b78480f5df88d33dfe9fbcd0f) Stream bulk-write progress events from dqlrs --serve.
- [c14d632](https://github.com/jspreddy/dql/commit/c14d632b36eb311de4180d25b31da195e9228340) Parse concatenated DQL JSON rows and tidy Jupyter config.
- [6612a67](https://github.com/jspreddy/dql/commit/6612a67faa10714ee5281fc45c5fe5eadf198091) Document the notebook UI and link it from the repo root.
- [1ab7fd2](https://github.com/jspreddy/dql/commit/1ab7fd2f2069de2f54128722f1a8adc9521645a0) Add getting-started notebooks for the Python Engine and dqlrs.
- [4ecc512](https://github.com/jspreddy/dql/commit/4ecc512b9916eb27db41eb1badb2ed08752b7c0d) Add JupyterLab project, DQL wrapper kernels, and start script.
- [e5f975a](https://github.com/jspreddy/dql/commit/e5f975a60448cc985ec7397813b23f7e59fa3566) Add plan for a JupyterLab notebook frontend.
- [de7164f](https://github.com/jspreddy/dql/commit/de7164f6bcda8dd0dcf2f04aef77de13c3dec409) Exit --serve --bind after shutdown and drop hanging bind subprocess tests.
- [9206d60](https://github.com/jspreddy/dql/commit/9206d600dcd7f2cae9b229dd3dc76eab7c16e005) Document dqlrs --serve and mark the headless worker plan implemented.
- [0356c72](https://github.com/jspreddy/dql/commit/0356c72f7e14fa01f1996dc0250dc43239b37220) Test dqlrs --serve over stdio, bind, and package smoke.
- [2b076c8](https://github.com/jspreddy/dql/commit/2b076c87203b034e082b5c44e4b5855ba00a42d8) Add dqlrs --serve JSON-lines worker with loopback --bind.
- [a486aef](https://github.com/jspreddy/dql/commit/a486aefde2e8062875d9dc71fe0f39bca804df93) Put loopback --bind in v1 of the dqlrs serve plan.
- [52c8110](https://github.com/jspreddy/dql/commit/52c811006519c6bd650734b5acd07bb9880c17de) Plan a headless dqlrs --serve worker for notebooks.
- [9de655a](https://github.com/jspreddy/dql/commit/9de655af43c11049a5c38cc4f77153cc93c92e5c) test(black-box-tests): fail the run when teardown DROP exits non-zero
- [aeaa282](https://github.com/jspreddy/dql/commit/aeaa282d544e3ff7a8d08bb00f7cb8a493373282) Complete implementation of verbose transcript features in black-box tests ([#24](https://github.com/jspreddy/dql/pull/24))
- [cce5137](https://github.com/jspreddy/dql/commit/cce513710ee3b5d700d509aea1bee6226dacb463) docs(black-box-tests): document -v CLI transcripts
- [3f3bad0](https://github.com/jspreddy/dql/commit/3f3bad0b19c949d1c7a259ba9f25a214a7bdf338) test(black-box-tests): print Rich transcripts when pytest is verbose
- [f7730d8](https://github.com/jspreddy/dql/commit/f7730d839e03cf5a30031b0abc46edc84743bc61) test(black-box-tests): add verbose descriptions to acceptance tests
- [7067f67](https://github.com/jspreddy/dql/commit/7067f67a324ac42b753d70df685a38136e7df27f) test(black-box-tests): add Rich transcript reporter
- [54be9da](https://github.com/jspreddy/dql/commit/54be9da9b5f1b1b7023c38bd2606c2579be707a6) test(black-box-tests): add rich for verbose transcripts
- [212d1f9](https://github.com/jspreddy/dql/commit/212d1f9965bc029dab763b081b2645652a4246c1) refine plan
- [038a684](https://github.com/jspreddy/dql/commit/038a684a5059aa3a0d26a64bcdf75d41f4b58d32) refine verbose output plan
- [2fb9759](https://github.com/jspreddy/dql/commit/2fb97591c468626d57ab19dfc6c7fffc1a305530) plan for verbose output
- [6d23796](https://github.com/jspreddy/dql/commit/6d23796fcef645a1e99517dd0ac0f7e91a7f3ca8) plan completed
- [b1cae81](https://github.com/jspreddy/dql/commit/b1cae81c183baa633f3c2aed94efbd34d726bd41) fix(black-box-tests): allow empty extra pytest args on bash 3.2
- [e7c0431](https://github.com/jspreddy/dql/commit/e7c0431fd9aa4cfdafd7cefb9afd7a4969b20952) docs(black-box-tests): document pytest harness and diagnostic groups
- [b047ac0](https://github.com/jspreddy/dql/commit/b047ac0004a9eaae8a36b4c89edaa53bc961eb36) test(black-box-tests): replace case folders with test_NNN pytest modules
- [0a7a13b](https://github.com/jspreddy/dql/commit/0a7a13be45c9b6d9902c3f3086eedd62c6203ec1) test(black-box-tests): add Cli helper and conftest for Local and both binaries
- [aa25dec](https://github.com/jspreddy/dql/commit/aa25decc25b94f15ff2a380309d6449529992356) test(black-box-tests): add pytest+pexpect project skeleton and run.sh
- [418252e](https://github.com/jspreddy/dql/commit/418252e9e608adac2f151795d2dac97632be33cb) refined plan
- [fcb8f3b](https://github.com/jspreddy/dql/commit/fcb8f3b7e7a51b8ee1915b625de181cc65f0a2b0) plan for acceptance tests using pexpect instead of custom bash harness
- [df4ecda](https://github.com/jspreddy/dql/commit/df4ecdae17e8a7368ae35f8e529fa8ee81606441) rename case step files with numeric order prefix ([#18](https://github.com/jspreddy/dql/pull/18))
- [83e1a01](https://github.com/jspreddy/dql/commit/83e1a0104588bcbad85a6c771bded5c92ff0202c) plan
- [5c84b22](https://github.com/jspreddy/dql/commit/5c84b2235fb94cf5f1e67740b5f27fe3c4a77bc8) Better isolate black-box table names ([#16](https://github.com/jspreddy/dql/pull/16))
- [f8957cf](https://github.com/jspreddy/dql/commit/f8957cf28b64d1977cb44f796fff806fc52cd9ff) skip teardown option
- [9005cc4](https://github.com/jspreddy/dql/commit/9005cc4a82fa5a28d278bd842324cec2f67a1c44) ci: register workflows without default-branch working-directory
- [ba6f761](https://github.com/jspreddy/dql/commit/ba6f761cf8574183e253f422f80dc22af5d795b7) fix(dqlrs): FilterExpression NOT, empty values, placeholder remaps
- [5d78928](https://github.com/jspreddy/dql/commit/5d7892810d68b950b2225ff8e4d6d717fca2af06) test(black-box): UPDATE all but two rows, SCAN attribute_exists count
- [0f76953](https://github.com/jspreddy/dql/commit/0f769538a249a885e20c4d50b965625b34210985) test(black-box): SELECT by pk, sk prefix, and extra filters
- [b0889e1](https://github.com/jspreddy/dql/commit/b0889e1961d8fb083c425e88483541d86303d8e8) Print hierarchical Setup/Input/Expect/Teardown in -v and fail on teardown errors. ([#15](https://github.com/jspreddy/dql/pull/15))
- [b5e6683](https://github.com/jspreddy/dql/commit/b5e66831926ce91088ff3606072f4683f78a0ed0) dont print empty result + alignment ([#14](https://github.com/jspreddy/dql/pull/14))
- [0302e50](https://github.com/jspreddy/dql/commit/0302e50c772a871c5312d178be7e73884097013a) pretty printing tweaks
- [2b86a23](https://github.com/jspreddy/dql/commit/2b86a235eef7b12de4700c2e120a6347d6046788) color attempt 1
- [3a4f101](https://github.com/jspreddy/dql/commit/3a4f10138e452bef27251176f287413c6d732ca3) feat(harness): enhance verbosity and color output in black-box tests
- [18f1472](https://github.com/jspreddy/dql/commit/18f1472d7701cf466ee6b223d7ca4f066159a747) re-ordering the black-box tests
- [a8a7cde](https://github.com/jspreddy/dql/commit/a8a7cde39184b6aa0dc317c218034a805728f5bf) test(black-box): add a CLI case for each statement family
- [9458a1c](https://github.com/jspreddy/dql/commit/9458a1c3e99c9f8dd5b28618eb158603d791f67f) fix(harness): run on macOS bash 3.2
- [86f7947](https://github.com/jspreddy/dql/commit/86f7947e5c8f75aee131d3f7cc29ee856006ae09) fix: reuse DynamoDB Local when the port is already open
- [bb546d3](https://github.com/jspreddy/dql/commit/bb546d30049aadd2febabce3ee360759580752a9) chore: rename manual-tests to black-box-tests
- [94c641a](https://github.com/jspreddy/dql/commit/94c641a09c03825688ec46c281b690d335492753) fix(manual-tests): read case files and normalize single-item JSON
- [f9e6637](https://github.com/jspreddy/dql/commit/f9e66373ed2fafb54b8a17ae0d5673a1f38d24f1) docs: point verification at black-box acceptance suite
- [0392a14](https://github.com/jspreddy/dql/commit/0392a1443626844688b77c25fbaba08890e748f2) test(manual-tests): add getting-started, select, and load seed cases
- [ef2c995](https://github.com/jspreddy/dql/commit/ef2c9956341c22eac116aa94822e736b264066e3) test(manual-tests): add black-box CLI harness for DynamoDB Local
- [9202899](https://github.com/jspreddy/dql/commit/9202899df6207ae2e820d655eb8c4d0cf7d94a38) chore(manual-tests): add acceptance case folder skeleton
- [d935882](https://github.com/jspreddy/dql/commit/d935882fa684ba55ae1878c28beea4200067e49e) docs(manual-tests): black-box acceptance test contract
- [8caae6e](https://github.com/jspreddy/dql/commit/8caae6e52336559a7d6156c8de53b606c9f7eb8b) docs(plans): black-box acceptance tests under manual-tests/
- [c4691c8](https://github.com/jspreddy/dql/commit/c4691c800924a9346a4ab54f6767da4aa56fe066) py dev docs update ([#11](https://github.com/jspreddy/dql/pull/11))
- [e806f3e](https://github.com/jspreddy/dql/commit/e806f3e18de1e30a9b2e1a04508b08a0975765af) Point callers at repo-root scripts and unify bump-version by language.
- [7fb54b6](https://github.com/jspreddy/dql/commit/7fb54b62474171ea4e7cd38fba7d43115335ec22) Move all helper scripts into a shared top-level scripts/ directory.
- [c9b49f8](https://github.com/jspreddy/dql/commit/c9b49f8fd94e89b1506fac9be011f3abd8af53c0) readme updates
- [559835d](https://github.com/jspreddy/dql/commit/559835d12b6b2a38a916812f2c86f3d71c237a98) readme update
- [29dcb1c](https://github.com/jspreddy/dql/commit/29dcb1ce40f5308c842f505c8e6a44177b4df46a) minor readme update
- [3d3f91e](https://github.com/jspreddy/dql/commit/3d3f91ee8d08c93bd984677a188494208e0ceff6) readme updates
- [02b0062](https://github.com/jspreddy/dql/commit/02b0062b2ed655a107debac21d2c44d94939e83a) Replace the mixed root README with a short Markdown index.
- [ea26679](https://github.com/jspreddy/dql/commit/ea26679af9a853a1c1548d92a8dc4e3fa1896be6) Run Python and Rust CI from path filters instead of v-next/v-rust branches.
- [5204b49](https://github.com/jspreddy/dql/commit/5204b4958bc9f85dc0df0cd73d7742964c961912) Let Python and Rust bump versions independently.
- [8e660ca](https://github.com/jspreddy/dql/commit/8e660ca9b73ebc4a77747263dcbc415c327b83f5) Give the Rust client its own user docs and install scripts.
- [07a5431](https://github.com/jspreddy/dql/commit/07a5431e6904346689895a5f321838b3f34a0111) Point Python packaging, Sphinx, and bump tooling at py-impl and py-docs.
- [992a707](https://github.com/jspreddy/dql/commit/992a707d26ad85f992d947404274939d8d040208) Add py-plans for Python-only design notes.
- [c97427c](https://github.com/jspreddy/dql/commit/c97427c634e0a7b32d2ff276d1c2f679d29eff8d) Move the Python-to-Rust migration guide into rust-docs.
- [c9f0993](https://github.com/jspreddy/dql/commit/c9f0993a4d2f016fec1afa026a0124fcac3b0e61) Move Sphinx docs into py-docs to sit beside the Python implementation.
- [7ab2f56](https://github.com/jspreddy/dql/commit/7ab2f5615ef7a2acb033e8e40bfb6830a12a4360) Move the Python implementation into py-impl so it no longer owns the repo root.
- [090a2c4](https://github.com/jspreddy/dql/commit/090a2c4f8668935084bacb3592542acdb664dcb8) dual plan refined to separate git mv vs code changes
- [0dcf008](https://github.com/jspreddy/dql/commit/0dcf008a0098007d2810ed7b854943226d20b9ec) dual implementation folders plan
- [32288c3](https://github.com/jspreddy/dql/commit/32288c3312f1f6d4b58bea01286689c82bb2afa0) docs: point clone, install, and source URLs at this fork ([#10](https://github.com/jspreddy/dql/pull/10))
- [d3bd7bd](https://github.com/jspreddy/dql/commit/d3bd7bdbf2e1bdcf8293781efeb394c4236d5fb9) wip: checkpoint
- [fb7adf2](https://github.com/jspreddy/dql/commit/fb7adf285aeb32e988a32962c5886591e6614974) rename rust binary to 'dqlrs'
- [6692dee](https://github.com/jspreddy/dql/commit/6692dee9502f1364cdd3ea0161bfdf5e457b6d04) Fix the prompt for localhost, cleanup
- [de3a1df](https://github.com/jspreddy/dql/commit/de3a1df7e9fa6089c8c795b7f769efb87d3292de) fix clippy warnings
- [a740c61](https://github.com/jspreddy/dql/commit/a740c61f884cf9533bc455cdc3c6a3495b0bd11a) fix(cli): use a top border on command panels instead of scrollback rules
- [34bfb7a](https://github.com/jspreddy/dql/commit/34bfb7a1c72e52544e236200ceaba410c254dd3f) feat(cli): show colorful grouped help for queries and meta-commands
- [e00fa0f](https://github.com/jspreddy/dql/commit/e00fa0f3e517268580540b62719685a892c9b35e) feat(cli): add history meta-command for list, search, dedupe, remove, and edit
- [1883f72](https://github.com/jspreddy/dql/commit/1883f72ff1a352d515e1821efccf69851bbd9926) feat(cli): multi-line prompt editing and smarter command history
- [530d5d1](https://github.com/jspreddy/dql/commit/530d5d13eadef48e794a3383c17f5b786e222b62) cargo fmt
- [f370b8c](https://github.com/jspreddy/dql/commit/f370b8c6747163e3f109929f7d01a1900412a79a) better terminal prompt & results
- [78c9c7b](https://github.com/jspreddy/dql/commit/78c9c7bbdaec56dc21a18df2390b6ae46a89cb76) fix(cli):  scroll is lagging but able to select text
- [286425f](https://github.com/jspreddy/dql/commit/286425ff3b89eccd02ef5c82368b90b1928304f2) Document SOLID design debt and refresh rust-impl README.
- [7645694](https://github.com/jspreddy/dql/commit/76456944af631769e0c83a78f7bb1bc1ca4a246a) rich format plan done
- [a73ea8f](https://github.com/jspreddy/dql/commit/a73ea8f88af0197afbe2cdd87d35e546195aa9ee) feat(cli): add rich query format with ratatui tables
- [d0ccd6b](https://github.com/jspreddy/dql/commit/d0ccd6b91ddf4535192a3f1a793c78a3eeee3cd2) rich output format plan
- [da88e62](https://github.com/jspreddy/dql/commit/da88e62d45402c75c4be78b96239b52a73060f24) chore(rust-impl): apply rustfmt
- [fb9d2df](https://github.com/jspreddy/dql/commit/fb9d2df507fabd695ff01db7279f51b3dfae3b92) feat(cli): shell-style REPL transcript with mouse scroll
- [d30cab0](https://github.com/jspreddy/dql/commit/d30cab0dc8128ad4bc05e88d46be046df157ba38) parity 6 plan done
- [7452b5c](https://github.com/jspreddy/dql/commit/7452b5cfc63cc6d3c4bc13cba874144c231c2a39) chore(tests): remove unused pending helper
- [af092c5](https://github.com/jspreddy/dql/commit/af092c53a1e7b6318e22ad82a4f0766283f02308) feat(cli): optional watch capacity dashboard
- [eb844af](https://github.com/jspreddy/dql/commit/eb844af1ce795809de0e4c0184b8fe7ec14b437b) feat(output): richer table detail for ls
- [70c7b88](https://github.com/jspreddy/dql/commit/70c7b88af02b7a41a36f07f6c3e67bf52f7b9cd1) feat(cli): CloudWatch metrics for ls
- [789f925](https://github.com/jspreddy/dql/commit/789f9259e0e52958cf52441c1a48ec041f2fa0a7) test(models): enable total GSI throughput parity test
- [cc24159](https://github.com/jspreddy/dql/commit/cc241596e1bf13679ce97e9826dd999ca2755044) fix(parser): populate ParseError offsets for caret display
- [760a5b5](https://github.com/jspreddy/dql/commit/760a5b500f5c4432d501b515655318cb0b366d90) fix(cli): honor display=less for command output
- [4e10577](https://github.com/jspreddy/dql/commit/4e10577b9fa15b9b63a8ad7d766c953af7f878db) parity plan 5 done
- [8a8e6ff](https://github.com/jspreddy/dql/commit/8a8e6ff3df412386a1dbc3c99e0623d437b1bbb2) test(engine): enable SAVE/LOAD format parity with MessagePack
- [adc5961](https://github.com/jspreddy/dql/commit/adc59619a0f797f0591e4b8cd412f270b80a3f58) docs: MessagePack replaces pickle for SAVE/LOAD
- [020d900](https://github.com/jspreddy/dql/commit/020d9003f37764a5d11c6d3670654993174d951d) feat(engine): MessagePack SAVE/LOAD as pickle replacement
- [306a99a](https://github.com/jspreddy/dql/commit/306a99a4f07a3df18e0822b210313e8afe786caa) feat(engine): SAVE/LOAD JSON CSV and gzip
- [bb9e7b4](https://github.com/jspreddy/dql/commit/bb9e7b480c2679c0269106fb8921fc78594e9372) feat(parser): parse SAVE clause on SELECT and SCAN
- [efd627b](https://github.com/jspreddy/dql/commit/efd627bbc8c9f4f2662bde39ac2e135792081d94) docs(plans): mark parity 4 expression regressions complete
- [667cb8f](https://github.com/jspreddy/dql/commit/667cb8f1d03c8d79f6ec6baa5c631249e155196d) test(engine): enable reserved-word and dashed-field regressions
- [3d77bf5](https://github.com/jspreddy/dql/commit/3d77bf5b2eb7fd545b5ac0bcf8a7f869e037c0ee) fix(memory): resolve dashed and nested fields in filters
- [4e96c01](https://github.com/jspreddy/dql/commit/4e96c014b2c88e872feb6a920222e1a4a4fdf0d4) fix(expr): use full DynamoDB reserved-word list
- [2d6caaa](https://github.com/jspreddy/dql/commit/2d6caaa7665fcc1b437d78be7bde40d515377250) feat(parser): accept hyphenated DynamoDB attribute names
- [dff90c9](https://github.com/jspreddy/dql/commit/dff90c9d9297a9c6d3d629752412361da954bd34) feat(engine): complete index projection and count-on-index parity
- [f43f236](https://github.com/jspreddy/dql/commit/f43f23646530ffab26fa0e657357b2615a1df168) feat(engine): complete ORDER BY / ASC / DESC parity with Python
- [5a0a8ca](https://github.com/jspreddy/dql/commit/5a0a8ca39b6f11cc5504eabc921d700301ad0762) fix(cli): default to AWS SDK backend when -H is unset
- [3451d7d](https://github.com/jspreddy/dql/commit/3451d7d09179e96909611f9d3d24dafea9914a94) parity analysis and plans ([#9](https://github.com/jspreddy/dql/pull/9))
- [6bd495c](https://github.com/jspreddy/dql/commit/6bd495c72684668297d0a9ecfe4836ab05c506ba) cargo fmt
- [7a99089](https://github.com/jspreddy/dql/commit/7a990898a54d38126f667eaa22afb40740745f2c) fix: output ordering
- [8f2d988](https://github.com/jspreddy/dql/commit/8f2d9886107bc4fa79a18de7bdfe88dfbf458910) fix: route REPL meta output through output pane and truncate ls names
- [6e6f4d5](https://github.com/jspreddy/dql/commit/6e6f4d53d18b468f2cd1abd02d0c2173d394d772) readme update
- [ed4f0aa](https://github.com/jspreddy/dql/commit/ed4f0aa7970f8ec7d089f30a163d439bc9f8e69e) correcting rust authorship
- [92653dc](https://github.com/jspreddy/dql/commit/92653dc0c61f1393cc777ee6f55a1a432f4bcdeb) chore: unify version bump across Python and Rust
- [04a3d74](https://github.com/jspreddy/dql/commit/04a3d740ea46c9bb9ce04a75cad4952a3aa579cc) docs: add Python-to-Rust migration notes
- [a8e1628](https://github.com/jspreddy/dql/commit/a8e1628f263b5e11eb00bb07b6a238dc5d7f95fc) docs: add Rust install script and cargo install instructions
- [fb86c47](https://github.com/jspreddy/dql/commit/fb86c4785971468d9ed228642c89e093f1aa62a3) ci: add GitHub release workflow for Rust binary
- [77a196c](https://github.com/jspreddy/dql/commit/77a196cf2bd0faf5fed0607068a5643be2ef841e) test: add package smoke test for release binary
- [94b198b](https://github.com/jspreddy/dql/commit/94b198b24de9c99e2fae7c9ded64cef0bf932735) ci: add Rust fmt, clippy, and test workflow
- [3e42148](https://github.com/jspreddy/dql/commit/3e421487999f506b6b27b65f6475741bb76f9ff6) build: add release profile and toolchain pin
- [d9adfee](https://github.com/jspreddy/dql/commit/d9adfeea4bc012633817762c2e0fd41311dac265) phase 6 plan: packaging and release
- [d4b5d5e](https://github.com/jspreddy/dql/commit/d4b5d5e607eecca3026b929f9a606db39ed4c2c4) plan 5 done?
- [72802ae](https://github.com/jspreddy/dql/commit/72802ae80ed46ba59a72102151dcff38ecd05a12) feat(cli): complete Phase 5 REPL, meta-commands, and parity tests
- [028a558](https://github.com/jspreddy/dql/commit/028a558810032acd34549620270e40d7c5d075f0) phase 5 step 1
- [81c50a2](https://github.com/jspreddy/dql/commit/81c50a2701ea18ce551d08ec7d6448b471b52608) original dql arch
- [91a47df](https://github.com/jspreddy/dql/commit/91a47df23d4e5365cbe800add7086f9661aa6814) moving the plan into .cursor folder
- [b8c571d](https://github.com/jspreddy/dql/commit/b8c571db3d04143480d0a6074303ad5ac08c248f) docs: add Phase 5 CLI, REPL, and output implementation plan
- [883fe25](https://github.com/jspreddy/dql/commit/883fe2526cf4d3fb1c56ff0ca6a4dda2b0d8f794) Phase 4: tests passing
- [dc93862](https://github.com/jspreddy/dql/commit/dc938627382b2ab800133f608bead1b1dbc6d834) docs: mark Phase 4 DynamoDB execution engine complete
- [b5d6379](https://github.com/jspreddy/dql/commit/b5d63798c461f5336811f5d0b8752e7a6fe09891) test(dql-engine): DynamoDB Local parity integration suite
- [e3695c3](https://github.com/jspreddy/dql/commit/e3695c3460670ac5b95a76c4c1308a6809d20480) feat(dql-engine): CSV LOAD support
- [47033ea](https://github.com/jspreddy/dql/commit/47033eafd10f70ee665505181f8e5b8a7172fc0b) feat(dql-engine): EXPLAIN kwargs, ANALYZE capacity, and THROTTLE
- [31a52ef](https://github.com/jspreddy/dql/commit/31a52ef336ddbfaca6033d01d9f288ee1c9a66b4) feat(dql-engine): DELETE USING/KEYS IN and INSERT keyword syntax
- [c23fb83](https://github.com/jspreddy/dql/commit/c23fb837b137a46ab813604e5cddf63e0346e4c1) feat(dql-expr,dql-engine): selection arithmetic evaluator
- [4058138](https://github.com/jspreddy/dql/commit/4058138f7aecdebdb06375a972b18458abd2ea04) test(dql-engine): FilterExpression parity for SCAN and SELECT
- [c4b94c1](https://github.com/jspreddy/dql/commit/c4b94c11fd0fc1b4ecc39263fe3732a3610bcec4) test(dql-engine): index planner and SELECT parity against Local
- [d1eb44f](https://github.com/jspreddy/dql/commit/d1eb44ff31df52e9d097d56c5c9fdef3e27dc559) feat(dql-engine): CREATE/DUMP index metadata round-trip
- [2c7227e](https://github.com/jspreddy/dql/commit/2c7227e3a5c4217c35fc703fee3e3c22514c291d) feat(dql-engine): complete UPDATE and ALTER parity tests
- [83c35c6](https://github.com/jspreddy/dql/commit/83c35c670afd2e699c54607d7a84c531d6191c37) feat(dql-engine): COUNT, projections, and consistent reads
- [9959fbb](https://github.com/jspreddy/dql/commit/9959fbb67cae02eda75766bf87c16e9343670cf4) feat(dql-engine): implement KEYS IN via batch_get_item
- [714ab77](https://github.com/jspreddy/dql/commit/714ab778250c081ec0c0ea4a6da5aacc2109436d) feat(dql-engine): wire USING index and scan rejection into planner
- [3a4edad](https://github.com/jspreddy/dql/commit/3a4edadb461463c5ccf1fdebc65fd090fbd9b489) feat(dql-parser): parse USING, KEYS IN, CONSISTENT, and ORDER BY
- [06e7640](https://github.com/jspreddy/dql/commit/06e76402931c1205b4971c2a702e4c936ed07ef4) test(dql-engine): add DynamoDB Local harness and update README
- [92ac4bd](https://github.com/jspreddy/dql/commit/92ac4bd905e416a23c4f2a21b3de8fe3aaf084fd) phase 4 checkpoint
- [ac9facc](https://github.com/jspreddy/dql/commit/ac9facc48d76e7164acdd5ec4d542feba96a1e7e) Document Rust migration Phase 4 progress
- [4e59834](https://github.com/jspreddy/dql/commit/4e5983403df46556e780e607a163c17aa62fcfc8) Wire engine reads through planner and expressions
- [360b0bf](https://github.com/jspreddy/dql/commit/360b0bf56efd0b3c07febc954839d92db65cc0bb) Add Phase 4 engine backend boundary
- [16964b9](https://github.com/jspreddy/dql/commit/16964b9c70ddf41a5733a993f05e322a7504a26e) Mark Rust migration Phase 3 done
- [92f312d](https://github.com/jspreddy/dql/commit/92f312d5df5380b469e705712ec6e8ecf635ed2c) Add Rust metadata and query planning crate
- [27b6115](https://github.com/jspreddy/dql/commit/27b6115889d455531929901ec07d7b8e1ccd87c3) Mark Rust migration Phase 2 done
- [3b66d1e](https://github.com/jspreddy/dql/commit/3b66d1e3d5f35ff6ef1ab26981718db956bc06d5) Fix expression crate lint
- [434b265](https://github.com/jspreddy/dql/commit/434b2657feaf0b2a14dd2b75d23eadaf5cdd0b41) Add Rust expression compatibility crate
- [b943018](https://github.com/jspreddy/dql/commit/b94301872b6dc44e769f766ef7d52a5b1734de7e) Mark Rust migration Phase 1 done
- [07596fc](https://github.com/jspreddy/dql/commit/07596fc4c3eed504845c85381aee3ed41a5b3160) Complete Rust parser Phase 1 parity
- [6e6b5a5](https://github.com/jspreddy/dql/commit/6e6b5a527551e3a7cefa9f7dd3c0ece3dee6aed8) Expand Rust parser Phase 1 AST surface
- [22cd173](https://github.com/jspreddy/dql/commit/22cd17340d91e7ffaecb438311f48e31de6fa127) Update Rust CLI plan to use ratatui
- [69721a3](https://github.com/jspreddy/dql/commit/69721a3a0141abecdeef1ee76efd716e905735ed) Document expanded Rust implementation slice
- [657a749](https://github.com/jspreddy/dql/commit/657a74996789b528af823957a0ed2f5c11c93b40) Add Rust in-memory delete support
- [61813da](https://github.com/jspreddy/dql/commit/61813da619f617f54c5955a15b2ae0c3bdb111de) Add Rust boolean where conditions
- [007159c](https://github.com/jspreddy/dql/commit/007159c61f9bdfb284e9c93005ad5b7ec1326bf5) Add Rust query limit and scan filter support
- [5b06ba5](https://github.com/jspreddy/dql/commit/5b06ba5ec6912f01462839d949e919a029b73525) Mirror Python tests in Rust parity suite
- [5c5b3bb](https://github.com/jspreddy/dql/commit/5c5b3bbb3743fe55deeffffe2ca881231380d414) Add initial Rust implementation workspace
- [51d7f58](https://github.com/jspreddy/dql/commit/51d7f58e1e59bde24e105cae99af89bec3e07e17) Add Rust rewrite planning docs

## dql (Python) 0.6.4-dev12 (2026-06-30)

Contributors: [Sai Jonnala](https://github.com/jspreddy)\
Agents: Cursor Agent

- [a5deed0](https://github.com/jspreddy/dql/commit/a5deed088fb02fb08c9eb550b02dfc58e540a484) Fix black formatting lint ([#8](https://github.com/jspreddy/dql/pull/8))
- [80db975](https://github.com/jspreddy/dql/commit/80db975e0e81fef85e6e11f4353485a182821a5c) Enhance bump-version script to handle git tags during commit amendments
- [00beeed](https://github.com/jspreddy/dql/commit/00beeed3aa395a1b305390f5e66f7358654a59e8) fix import ordering with pyparsing_compat
- [6189b94](https://github.com/jspreddy/dql/commit/6189b943f16bec91ace96ba9ae9848bb5b19fee1) changelog
- [f8d7732](https://github.com/jspreddy/dql/commit/f8d773283ad7a8a16a929b2c58873765ae6051ff) Update GitHub Actions workflows to use latest action versions
- [40628a2](https://github.com/jspreddy/dql/commit/40628a2b85a8d9b6c9907244a950d2ab6cf17ee6) lint issues
- [ffa79a7](https://github.com/jspreddy/dql/commit/ffa79a762857cf009d11276b0ccb433a16f74b02) lint fix
- [7de3d74](https://github.com/jspreddy/dql/commit/7de3d7495c960151b534f6ed8a9302d5cdca8535) Add *uv run task test-matrix* command for multi python test
- [1203e11](https://github.com/jspreddy/dql/commit/1203e11c286c6e61838b4cd55e4496362651863f) uv migration + py [3.10, 3.11] compat
- [6bd8f36](https://github.com/jspreddy/dql/commit/6bd8f367604407bb016841f310c27b545cc438d9) Remove left over clutter from poetry optional dependencies
- [21feb1a](https://github.com/jspreddy/dql/commit/21feb1a923162d7ca5e63fc64e28627a5d30b630) Refactor dependency management in pyproject.toml by replacing dev-dependencies with a dependency-groups section for improved organization.
- [0917e1a](https://github.com/jspreddy/dql/commit/0917e1ac537bc3aa1797ac403c140463ecff50e6) Update test-specific command in pyproject.toml to simplify usage by removing argument placeholder.
- [8936129](https://github.com/jspreddy/dql/commit/8936129a247596bbe7f468948709ffaf9fb651f5) Fix task bump argument passing for bump2version.
- [68de554](https://github.com/jspreddy/dql/commit/68de5541f14663207450ec5f1d958b76213c4a18) Update project configuration for versioning using bump2version
- [bcfc76c](https://github.com/jspreddy/dql/commit/bcfc76cabf8c1970138298156ca777f66f683460) Add error handling for readline history length retrieval and corresponding test case
- [616c10b](https://github.com/jspreddy/dql/commit/616c10bd09d94b6c8f6fac484172fe25c4926eee) fixing issues raised by agent review
- [ac47b7d](https://github.com/jspreddy/dql/commit/ac47b7d6702419b6d37c72cbc248e9eb5859b7b2) fix some lint issues
- [a08d76c](https://github.com/jspreddy/dql/commit/a08d76ce7b8ad29a180b87524a8a25344b2e7a3c) Fix readline history by sharing gnureadline via sys.modules ([#6](https://github.com/jspreddy/dql/pull/6))
- [19435b8](https://github.com/jspreddy/dql/commit/19435b8a700d69d2bcf6f2538a35a82565d6a2fd) wip: why is readline not working properly
- [eb2412a](https://github.com/jspreddy/dql/commit/eb2412a2d63c8b20e14449b952378cb81051e543) chore: tasks
- [111ccf3](https://github.com/jspreddy/dql/commit/111ccf35482800b259eaf8e84a8b5da29a26e321) lint fixes
- [1ab30bf](https://github.com/jspreddy/dql/commit/1ab30bfa6cb1d9834f9fec71135dc7aa57e810bf) changelog updates
- [f944eb7](https://github.com/jspreddy/dql/commit/f944eb7e16f394b1dace36baf0e156f1ff784de1) changelog update
- [7cd69c7](https://github.com/jspreddy/dql/commit/7cd69c702d84a43e69340649871691b69a8b3ff6) lint fix
- [2501f5d](https://github.com/jspreddy/dql/commit/2501f5d399b84f15c07db4d0cd2881935411ed84) changelog
- [850b08a](https://github.com/jspreddy/dql/commit/850b08a1f5c5d0a0c88458c09d5745402b48d7e2) install gnureadline to fix flakey builtin python module of readline
- [967efb1](https://github.com/jspreddy/dql/commit/967efb15368de29652015bd191354759ea82d5a3) cleanup
- [660fed6](https://github.com/jspreddy/dql/commit/660fed6da5628d75885781db038b485cbf2ee75b) remove poetry
- [aad681c](https://github.com/jspreddy/dql/commit/aad681c0174bca9bf797e9151003824eb6359ea8) backup
- [59a3ed5](https://github.com/jspreddy/dql/commit/59a3ed57491adf07f7528bb74880fa4843321374) wrap history read in try
- [0e6fb4e](https://github.com/jspreddy/dql/commit/0e6fb4ea9b1ef308ed4fdc432111c019c37de1ed) cleanup
- [fca8f2f](https://github.com/jspreddy/dql/commit/fca8f2fd65422de1d4a3cfa444e08c90c1345527) migrate all tox scripts to uv taskipy tasks
- [ca5ac33](https://github.com/jspreddy/dql/commit/ca5ac333759bd900c7595b1882c411f0ccbe11d2) fix attempt 1
- [0955afb](https://github.com/jspreddy/dql/commit/0955afb75035152addc9d253afdc3c26ad693d83) Agent changes: migrate to uv

## dql (Python) 0.6.4-dev10 (2025-12-05)

Contributors: [Sai Jonnala](https://github.com/jspreddy)

- [f8ede16](https://github.com/jspreddy/dql/commit/f8ede16688c32c183a83576a4a3e215a5c082aec) changelog ([#5](https://github.com/jspreddy/dql/pull/5))
- [f6959a9](https://github.com/jspreddy/dql/commit/f6959a974bba5ee79debef2b43d66e4049236ffb) If the first available matching index is table, use it.

## dql (Python) 0.6.4-dev9 (2025-08-01)

Contributors: [Sai Jonnala](https://github.com/jspreddy)

- [26d99f3](https://github.com/jspreddy/dql/commit/26d99f31755c5aeaf018847494e46611f687e4d5) fixing error ([#3](https://github.com/jspreddy/dql/pull/3))
- [913a33f](https://github.com/jspreddy/dql/commit/913a33fb3cf2f51226b087f82e46ed7a5de0c440) flakey typing is bad
- [adddaae](https://github.com/jspreddy/dql/commit/adddaae5b9ced11bb3a601d27b74bfda99ed7712) lint fixes
- [a306375](https://github.com/jspreddy/dql/commit/a306375fa2747b8bb09e5b7a69e5dd132a90b90f) use latest github runner
- [0a6dcc8](https://github.com/jspreddy/dql/commit/0a6dcc8c12f5a4ed574cb83cee149cc0d8a96c57) bugfix: boolean keyword not parsing properly
- [00b350d](https://github.com/jspreddy/dql/commit/00b350d0b3bbaad8c337dfbbfc4e3824980d530f) rich format: limit cols to display; print all col names in result;
- [cb3ab3c](https://github.com/jspreddy/dql/commit/cb3ab3c41353d80aeb674f000e93d6ec14d4ee94) combination script
- [a4bfae8](https://github.com/jspreddy/dql/commit/a4bfae882bfd5d3d7ad781e7dc8db25e25a3f2ae) download dynamodb local if not exists
- [7790973](https://github.com/jspreddy/dql/commit/779097379c0faa94d4578c16ae57f33246b343b7) formatting of changes.rst file

## dql (Python) 0.6.4-dev8 (2024-12-19)

Contributors: [Sai Jonnala](https://github.com/jspreddy)

- [e9a4366](https://github.com/jspreddy/dql/commit/e9a4366bebd40fc83485dccab4e9dc72f6187e37) changelog ([#2](https://github.com/jspreddy/dql/pull/2))
- [a62b0b9](https://github.com/jspreddy/dql/commit/a62b0b9e2026a53354dc0f6dfbc1145809c50486) minor cleanup of tox.ini and help.py
- [cc8962d](https://github.com/jspreddy/dql/commit/cc8962d0274d99953cfb5978d905cfe984174541) ci try
- [17821b5](https://github.com/jspreddy/dql/commit/17821b53f244d363efe3e9ce026960d8bea84791) ci try
- [32a4f17](https://github.com/jspreddy/dql/commit/32a4f17c31a14ff177ab6e268f9c89207c877e7c) ci try
- [7209af5](https://github.com/jspreddy/dql/commit/7209af5b11e53f348291f779778914a234b3ffff) ci try
- [e182fd0](https://github.com/jspreddy/dql/commit/e182fd043965c46a7c6d294fc03c9d41b50276a8) try code workflows with java
- [24a0454](https://github.com/jspreddy/dql/commit/24a04547c605ee091a72fcc7395571653a06e8d5) more lint fixes
- [dd83d05](https://github.com/jspreddy/dql/commit/dd83d051890435b92f5e7d2e5d3b6dc69e1665af) pylintrc disable variable renames
- [36c026c](https://github.com/jspreddy/dql/commit/36c026c85dcc0922ba8308eb142d42dbadfa03eb) mypy lint fixes
- [efd9d64](https://github.com/jspreddy/dql/commit/efd9d6460c5ee070bdbb65e23d3574ed22e5a801) lint fix
- [6b02b56](https://github.com/jspreddy/dql/commit/6b02b561d4595f400481ff0eb44c7ac305928fe7) format
- [9627ba4](https://github.com/jspreddy/dql/commit/9627ba426db1f520de491a3dce6edc3ea3f4c12c) fixing lint + test
- [e7df60a](https://github.com/jspreddy/dql/commit/e7df60a3e003ef87ebabd6f601b6909c2e03cef7) "cleanup"
- [0ffc3a1](https://github.com/jspreddy/dql/commit/0ffc3a1086c158adef5557719ad49a2aed08c13f) lint fix
- [3abff2e](https://github.com/jspreddy/dql/commit/3abff2edabc9a9369346c68831d87d3d0890c1ff) BugFix: save as binary mode: error due to encoding being specified
- [c5f0189](https://github.com/jspreddy/dql/commit/c5f0189681b4f053aaf15a68e42f3e8160945894) skipping a failing test due to dynamodb-local bug
- [7900be3](https://github.com/jspreddy/dql/commit/7900be3078df9f96bbb30291f704148a791d82e8) somewhat working tests
- [eb89c28](https://github.com/jspreddy/dql/commit/eb89c285919e61862ba775bfe08a80d17f1cff1a) test_cli: Pytest update, syrupy update - replace snapshottest with syrupy, - update tests to work with pytest - some lint fixes
- [8e42674](https://github.com/jspreddy/dql/commit/8e42674566cacce0e311e5de2230ba13a0f98b7c) more fake user data
- [255f75e](https://github.com/jspreddy/dql/commit/255f75e1791f0b463088ccbf040b4fead6cb3aac) RichFormat: preserve order of important cols
- [ba2364b](https://github.com/jspreddy/dql/commit/ba2364b3c2e2145b902e83cceaf0fe17488b90e1) lint fix
- [4ef29ab](https://github.com/jspreddy/dql/commit/4ef29abc8254d047186bb493c87a720684c53875) BugFix: analyze reprinting results over and over
- [75fb695](https://github.com/jspreddy/dql/commit/75fb695772d7e39b0fc26a722f0791900ca53f74) Adding a rich format
- [568e527](https://github.com/jspreddy/dql/commit/568e527248923952cf7476cea60010b5ec15a5e7) adding manual test data
- [261d703](https://github.com/jspreddy/dql/commit/261d70371088df5e7116b9e07bc8c23a881ac8a8) adding version, iam commands to cli
- [bc22630](https://github.com/jspreddy/dql/commit/bc226308253fd2d66de4de6545dfe7324508bc14) lint fix
- [e94ac64](https://github.com/jspreddy/dql/commit/e94ac643a944502c52b29eb783e1d61582f3d8fd) adding c command as alias to clear.
- [f8d8580](https://github.com/jspreddy/dql/commit/f8d8580d9a7b7275bb2317f7b8f5798757307aba) Moving functions around into categories.
- [08fa8a1](https://github.com/jspreddy/dql/commit/08fa8a11b9ce5e22ac2b7fd0a8eb9370d2266f58) github job changes
- [1863d23](https://github.com/jspreddy/dql/commit/1863d23205b214b6aff52be03ae7b3acef14bd1a) readme updates for v-next

## dql (Python) 0.6.4-dev6 (2024-10-17)

Contributors: [Sai Jonnala](https://github.com/jspreddy)

- [00a1be2](https://github.com/jspreddy/dql/commit/00a1be28474566c5032bc276ba524e8c9f9c8a34) try again to fix packaging issue.

## dql (Python) 0.6.4-dev5 (2024-10-17)

Contributors: [Sai Jonnala](https://github.com/jspreddy)

- [5b8b042](https://github.com/jspreddy/dql/commit/5b8b042b580bf41ce7f981e18dc0ba2518dba52f) Fixing packaging with pyproject.toml
- [27cbdfe](https://github.com/jspreddy/dql/commit/27cbdfec8e91c413765b752132d2a46c28cd82a4) readme
- [65dfd68](https://github.com/jspreddy/dql/commit/65dfd6816b50549fdf98ac82b474fc1ce2b3fa91) Readme updates

## dql (Python) 0.6.4-dev4 (2024-10-13)

Contributors: [Sai Jonnala](https://github.com/jspreddy)

- [0308870](https://github.com/jspreddy/dql/commit/0308870a24a1156b04c407f62fafb6a451360e26) Fixing some cli prompt bugs.

## dql (Python) 0.6.4-dev3 (2024-09-28)

Contributors: [Sai Jonnala](https://github.com/jspreddy)

- [131ecb2](https://github.com/jspreddy/dql/commit/131ecb28e27e1bb6e1ea9f3ac7ba7dcba2dbb5f8) Most tests cases are passing with local dynamodb

## dql (Python) 0.6.4-dev2 (2024-09-28)

Contributors: [Sai Jonnala](https://github.com/jspreddy)

- [77fb8da](https://github.com/jspreddy/dql/commit/77fb8da9803e7641afb35d8c332268411e44a4c9) removing setup.py and migrating to pyproject.toml completely
- [77f5014](https://github.com/jspreddy/dql/commit/77f5014a82a736ec5ccd2b310356bef0f8215de0) minor code formatting
- [6a70572](https://github.com/jspreddy/dql/commit/6a705720a5014887d0aad2542b9ee34f17b67dc9) adding command whoami to print the session identity.

## dql (Python) 0.6.4-dev1 (2024-09-21)

Contributors: [Sai Jonnala](https://github.com/jspreddy)

- [c1f38fa](https://github.com/jspreddy/dql/commit/c1f38fabc24c4fe79290903a9d0f552b20595475) BugFix: <LS> command breaks if using semicolon. - "ls tablename;" caused error on semicolon. Fixed it.
- [056c365](https://github.com/jspreddy/dql/commit/056c3658c3fc439ab4d8f62ec02a8cc4e72a4ded) trying to modernize the build / test tooling
- [1bc17ab](https://github.com/jspreddy/dql/commit/1bc17ab0502c0398be3e49af965400c804028c56) Developer/maintainer guide link on main readme
- [8a90990](https://github.com/jspreddy/dql/commit/8a90990b72c8a1eea60de3641a18d2bdaff1cc16) maintainer install notes
- [9418b4a](https://github.com/jspreddy/dql/commit/9418b4a7bd422ea29d4d4ea53faa09b6bc754b73) lint fix

## dql (Python) 0.6.4-dev0 (2024-07-30)

Contributors: [Sai Jonnala](https://github.com/jspreddy)

- [9ee27ab](https://github.com/jspreddy/dql/commit/9ee27abe0f0368d0983ee5a52e6fc39136668744) add ability to force "no index" when querying
- [7d4e5d5](https://github.com/jspreddy/dql/commit/7d4e5d5026b07eb6bef58c133e1e0d62c832c10c) attempt to fix not selecting index
- [3c8a508](https://github.com/jspreddy/dql/commit/3c8a508db882d703648d2bb416473b3c3c794566) re-using progress bars

## dql (Python) 0.6.3-dev1 (2024-05-24)

Contributors: [Sai Jonnala](https://github.com/jspreddy)

- [8ed003c](https://github.com/jspreddy/dql/commit/8ed003cb8c57d31a04d1cec35b0f21042217512c) lint fixes for test cases
- [3eccf1a](https://github.com/jspreddy/dql/commit/3eccf1a82f7f4cedc3d579d0fbe7f626fc4f7139) Adding progress bar to batching operations
- [f77f53f](https://github.com/jspreddy/dql/commit/f77f53f5813c058c3e3b728e1344e64bc2a4fcab) Fix errors with _attributes in query
- [d1fc4a6](https://github.com/jspreddy/dql/commit/d1fc4a68cb3b53880e1f0e6b820ad5ececf0a500) fixing some lint issues
- [4159f43](https://github.com/jspreddy/dql/commit/4159f43f06afdf8cb613edbb068fad88f5319eff) formatting using tox -e format
- [5b26679](https://github.com/jspreddy/dql/commit/5b266798a843a43caf0348aefd0a7f7cabd56357) Fixing some bugs, upgrading python, tox and other deps
- [3d3ead6](https://github.com/jspreddy/dql/commit/3d3ead68f2574b212bfc4dce10e2ff336a15b11c) wip: ls command: rich rendering + updates
- [e6bc1cd](https://github.com/jspreddy/dql/commit/e6bc1cdcd3ccf7e0d31f51b15ea3b2e35d2645c1) wip: ls command: partial rich rendering

## dql (Python) 0.6.3-dev0 (2021-12-23)

Contributors: [Sai Jonnala](https://github.com/jspreddy), Steven Arcangeli

- [7e28f19](https://github.com/jspreddy/dql/commit/7e28f190d8d2c21a9a2ea500625d82a9d92ac9a6) wip: colorizing JsonFormat
- [9666cfb](https://github.com/jspreddy/dql/commit/9666cfba19773c20c7b4be29adc7d6179cf00d44) doc: forgot to remove 'unreleased' from 0.6.2

## dql (Python) 0.6.2 (2021-12-23)

Contributors: [Sai Jonnala](https://github.com/jspreddy), Steven Arcangeli

- [d8c2862](https://github.com/jspreddy/dql/commit/d8c28621d9c378b6416f87c6b5863c573861096a) doc: update readme installation instructions
- [b0231a0](https://github.com/jspreddy/dql/commit/b0231a041a2235338b09fef28eaaaea698d8db72) fixing error in conditional import of readline (#43)
- [949dee9](https://github.com/jspreddy/dql/commit/949dee90470b8ba63828130b540e97be6a105d4e) fixing error with exiting via shortcut
- [6941852](https://github.com/jspreddy/dql/commit/69418527ebea2bb0bebbbeefdb6d7871abbe6e18) doc re-generate
- [0611643](https://github.com/jspreddy/dql/commit/06116434c8f34edaa644fb2ae45f078611708324) help docs for clear and cls
- [69af181](https://github.com/jspreddy/dql/commit/69af181645b3b3c89fa45d962e80a1a33636c87a) Code formatting
- [6005004](https://github.com/jspreddy/dql/commit/6005004ceca76c64aa2bca24d8cb266e49429dc2) Adding clear command and excluding clear, exit from history
- [4951ffe](https://github.com/jspreddy/dql/commit/4951ffe3eb75c93176f0267330106b87ef2214f3) chore: configure workflow to auto-publish on tag (continues #42)
- [9383e32](https://github.com/jspreddy/dql/commit/9383e3208edbe762ead499c288fead81e1ae3a46) doc: fix title underline in changelog
- [50574c2](https://github.com/jspreddy/dql/commit/50574c2f45a8de2de5f880136d1896b6c5c87b96) doc: update the build badge in the readme

## dql (Python) 0.6.2-dev0 (2021-12-06)

Contributors: [Sai Jonnala](https://github.com/jspreddy)

- [fbc2b49](https://github.com/jspreddy/dql/commit/fbc2b49e1be397c99094072dc6423d523a0adaba) Dont need to build on each push (#42)
- [98a7c47](https://github.com/jspreddy/dql/commit/98a7c47783b5c97de4c3053a74aa8d1fc378a9f0) run the workflow on pull_request
- [290b473](https://github.com/jspreddy/dql/commit/290b47380c40491c32c1b75f1ea2cad4a6b729a6) cleanup + changelog
- [25adebf](https://github.com/jspreddy/dql/commit/25adebffe5c5e0654651f2e52ce660ceaae8aaae) wip: revert publish job
- [c0d72f4](https://github.com/jspreddy/dql/commit/c0d72f4e09b4b72348fb2ba61474c46f4cf250ce) wip: publish job
- [4cee4cb](https://github.com/jspreddy/dql/commit/4cee4cbae753a2acdd30b7d781cd747e8021fc7e) wip: build-job & artifact
- [68fcfb1](https://github.com/jspreddy/dql/commit/68fcfb19764f11acfc7dd5717cd152c8bf4840bc) wip: github actions
- [6c72eb5](https://github.com/jspreddy/dql/commit/6c72eb574b1317da2fde58350cd282d9b18ed73a) wip: github actions
- [2ab332f](https://github.com/jspreddy/dql/commit/2ab332f2871f038e12a11a67530f225d6fd37de2) wip: github actions
- [0749c57](https://github.com/jspreddy/dql/commit/0749c57329b03417e162aace4cddc846668e299c) wip: github actions
- [d9569cf](https://github.com/jspreddy/dql/commit/d9569cf2e88d23799e4643280ad27866305cfa94) wip: github actions
- [01e8d99](https://github.com/jspreddy/dql/commit/01e8d991a24999d77e84e91a656dcc5507b2a7ca) wip: github actions
- [3471385](https://github.com/jspreddy/dql/commit/34713851cbeee67584af16f9e56e39e93ee5582c) wip: github actions
- [c403114](https://github.com/jspreddy/dql/commit/c4031147c227c25b95f1cf8627d5d4bfd66b56e2) wip: debug github workflows
- [e21835f](https://github.com/jspreddy/dql/commit/e21835fe07c79ca28bea3cb43bb098e6a4e13ebb) testing github actions
- [63d9632](https://github.com/jspreddy/dql/commit/63d9632e77623cde681ba28a268f64a00651fe8d) Fixes, Chores, Additions, see changelog
- [377e250](https://github.com/jspreddy/dql/commit/377e250d0c11c41be1ce0ab0603f44e56df0207a) chore: black formatting

## dql (Python) 0.6.1 (2021-01-24)

Contributors: Steven Arcangeli

- [52290e1](https://github.com/jspreddy/dql/commit/52290e15d2fd47510339548f5a663e1e70737cbe) Updating changelog
- [0656b25](https://github.com/jspreddy/dql/commit/0656b255a396cb1a2e3ebdfe8074e7903217ecf4) Clean up history test
- [6e3fb27](https://github.com/jspreddy/dql/commit/6e3fb2721c987cae6b1ddd5d2ff52bb47eaab64d) Add .envrc for direnv

## dql (Python) 0.6.1-dev1 (2021-01-21)

Contributors: [Sai Jonnala](https://github.com/jspreddy), Steven Arcangeli

- [1704b83](https://github.com/jspreddy/dql/commit/1704b83bb2781b8a50aaeea681c31d08146ff32b) fixing test (#41)
- [d14683d](https://github.com/jspreddy/dql/commit/d14683db1ed74ff33f9b0783e12e0c76a1971e60) Basic Unit tests for the history manager.
- [7ae26d4](https://github.com/jspreddy/dql/commit/7ae26d4d549424ee17fba92afa28028db83b7721) cleanup print
- [a83baff](https://github.com/jspreddy/dql/commit/a83baffacca0a80065b41dabe3d95c992674fb62) Raising exception when write history failure
- [20d7f72](https://github.com/jspreddy/dql/commit/20d7f721ccf9fc167b94764fdb2e1a01ed39f57e) only trying to write history when it is > 0
- [b6ffad6](https://github.com/jspreddy/dql/commit/b6ffad62150364e498c6ddfb2b61ba6c98c978e5) adding ls snapshot test for listing multiple tables
- [35e5dda](https://github.com/jspreddy/dql/commit/35e5ddab520896552be5bcd4d1de826114ed0a90) cleanup
- [aff98f1](https://github.com/jspreddy/dql/commit/aff98f12cb3f62dbb590c7913572d95e1e16989d) Extracting the history management into its own class
- [7b2419f](https://github.com/jspreddy/dql/commit/7b2419fdf26861f40fb993da1f62faa862795190) Fixing history loss when using multiple sessions.
- [cdfb0e6](https://github.com/jspreddy/dql/commit/cdfb0e61743119d80ee273c9c66b97c88909381d) fixing mypy lint issue
- [5fefcac](https://github.com/jspreddy/dql/commit/5fefcac5c0c629dd747eb6503ee5e95719560df4) fixing lint issues
- [f16ede7](https://github.com/jspreddy/dql/commit/f16ede71707e021bda54bb2d0b4b4c5a671076af) changelog
- [992e054](https://github.com/jspreddy/dql/commit/992e0543805b4f99f5b233dab31e2a360eebf4c3) Feature: Retain query history across sessions.
- [045e601](https://github.com/jspreddy/dql/commit/045e60188a0db0bbfd3097ce46879c3e51358764) Minor refactor + Function Documentation
- [d37f07f](https://github.com/jspreddy/dql/commit/d37f07f04a588d55d889e4da0aa7cf5827c29e2f) Add a note about PartiQL in the README

## dql (Python) 0.6.1-dev0 (2020-11-21)

Contributors: [Sai Jonnala](https://github.com/jspreddy), Steven Arcangeli

- [e235426](https://github.com/jspreddy/dql/commit/e235426b1e32df8139d0b4e12f3c7fe237070371) Tracking for new release + Change log (#38)
- [000175f](https://github.com/jspreddy/dql/commit/000175f7369ded4129bd679912d78aa856e5c781) Dynamo local path to be within project root + gitignore
- [4706ad7](https://github.com/jspreddy/dql/commit/4706ad7762a2027db3bbf21da6f4ef915f8c6c6b) Fix #36: Constraint functions should accept quoted field names
- [3c95123](https://github.com/jspreddy/dql/commit/3c95123566723d4e9ef863583f81cec5a704a1f3) Parser extracts values during parsing for conditional constraints
- [e245229](https://github.com/jspreddy/dql/commit/e2452294b68e1e117e73e5949a5cb1b9e817c45f) Change the parsing logic for constraint expressions
- [03cfea1](https://github.com/jspreddy/dql/commit/03cfea1f361b87dd44cea14674265f8ce3785aae) Fix save and load file logic
- [4c6fb35](https://github.com/jspreddy/dql/commit/4c6fb35795a56d70beae3d0c1b7400987e4f4121) Fix #37: Cannot count(*) on an index

## dql (Python) 0.6.0 (2020-10-03)

Contributors: [Sai Jonnala](https://github.com/jspreddy), Steven Arcangeli

- [01bfeb3](https://github.com/jspreddy/dql/commit/01bfeb377a8a81bb2dd8f8d952d6a9f2efd24a8f) Updating changelog
- [8d18520](https://github.com/jspreddy/dql/commit/8d18520030e7d627cfa86c946cbc3104c6ce27ed) Update dynamo3 requirement
- [4f305c4](https://github.com/jspreddy/dql/commit/4f305c4088a60b34f307197ef665c8141c1aba7c) Fix formatting
- [dffa84e](https://github.com/jspreddy/dql/commit/dffa84e11a0c3af7c169159632786c6e6490801c) Updating changelog
- [ef604ea](https://github.com/jspreddy/dql/commit/ef604ea160dd2107bb9cb88b86c27c16e595fa95) Fix #34: ls on a table with global indexes causes AttributeError
- [9900a8a](https://github.com/jspreddy/dql/commit/9900a8a15f201b61d42319bf6c0ac6534ee7572d) Add snapshot test for ls format
- [bfa6ea7](https://github.com/jspreddy/dql/commit/bfa6ea731003802fd99fb1ad81c95097442efcf4) Add a --json argument to output queries in line-by-line json
- [6a30999](https://github.com/jspreddy/dql/commit/6a30999845d92c05c3b55b4d1b6c6b83c3c58c14) Add new formatter for JSON output
- [33f9443](https://github.com/jspreddy/dql/commit/33f944338113bb46381d6d8fb49fe95f1fc2ea4f) Update changelog
- [736a050](https://github.com/jspreddy/dql/commit/736a050cff42248041d3d2852e8aea755a2ee906) adding new line so that the display is correct. (#33)
- [bfaef96](https://github.com/jspreddy/dql/commit/bfaef969ca41781807b5e7935e691199e1eca0c5) Fix tests and lint
- [99d0216](https://github.com/jspreddy/dql/commit/99d021616af5bc131fdb7e014393ca6fff38a2df) Fix #30: ls command accepts glob patterns
- [3cb64a1](https://github.com/jspreddy/dql/commit/3cb64a17688d81275ce3073dedfc7d655cde8311) Leave the stack trace out of the parse exception display
- [aa86dd3](https://github.com/jspreddy/dql/commit/aa86dd30eb4e10b17c042bca5263f29972b760ba) Upgrade dynamo3 to pre-release version
- [79f3f8b](https://github.com/jspreddy/dql/commit/79f3f8b838a6d9fa87f4c8d25fe175fa79a2d378) Tracking the changes in CHANGES.rst (#32)
- [a5c0a18](https://github.com/jspreddy/dql/commit/a5c0a18c8bf93b9727b6f2c6c9a27fea20c9d28c) Fixed #31: On-Demand tables cause ZeroDivisionError with ls
- [550c86d](https://github.com/jspreddy/dql/commit/550c86d706bd82517b8fe0dd2ee2fd9a5469c37d) Enable mypy checking for untyped defs
- [374e736](https://github.com/jspreddy/dql/commit/374e736baef35a2112ec8fe6e5a3eec461e9c5c9) Add mypy as a test requirement
- [5e73670](https://github.com/jspreddy/dql/commit/5e73670f29d21eed1d96d4a74dbd73e818d6aad7) Make the mypy checking slightly stricter
- [667067c](https://github.com/jspreddy/dql/commit/667067cb3b07df0d1cb48bcaa1a96a328e96315d) Replace old assertItemsEqual method with assertCountEqual
- [9e54053](https://github.com/jspreddy/dql/commit/9e540531a17c43247c06e0c4acc0f1ee60529808) Add mypy checking in lint step
- [f997bff](https://github.com/jspreddy/dql/commit/f997bffcf6d1cd9f239a04ae06f0e3c9c200c17a) Use requirements files to manage test/dev dependencies
- [b82153c](https://github.com/jspreddy/dql/commit/b82153c8e6bdec3a9fc296a223f3d6247df7bd80) spooky season updates (#28)
- [d9a5c05](https://github.com/jspreddy/dql/commit/d9a5c05dcec88fbadd7e63669dff05ad8fdc68b5) Refactoring the with_exception_handling using  @contextmanager
- [263e6d6](https://github.com/jspreddy/dql/commit/263e6d6880546363e1150ddc3693b4392d5377cc) setting up a coverage tox env
- [e82a3b4](https://github.com/jspreddy/dql/commit/e82a3b40ab49673c2cd05617f8244b6df6c37ae4) Handling exceptions via a wrapper.
- [cc9e952](https://github.com/jspreddy/dql/commit/cc9e95281834a909fb812d0d002489576cf6c6ba) Cleanup of unused import
- [24d32be](https://github.com/jspreddy/dql/commit/24d32bea20b6d78eebd7083b3a57608a71d88f78) Error handling for -c option + Rich errors.
- [bce6d7d](https://github.com/jspreddy/dql/commit/bce6d7d4e0fcc4983acccf6faf62bbe025fc89bf) Adding .editorconfig
- [93f430b](https://github.com/jspreddy/dql/commit/93f430b8e68621d2da9c1d9566f90eee4843791a) Adding `rich` and `botocore` to the requirements.

## dql (Python) 0.5.29-dev0 (2020-09-06)

Contributors: [Sai Jonnala](https://github.com/jspreddy), Steven Arcangeli

- [4686799](https://github.com/jspreddy/dql/commit/46867998198575df2b928d2da24194a8f90447de) CI is finally working with the proper java version (#1)
- [524391b](https://github.com/jspreddy/dql/commit/524391b92521a75f39b825e8f43dcdbf1aae04f2) a new hope
- [f2f0b50](https://github.com/jspreddy/dql/commit/f2f0b501bef3b01761efa4ed41c4aeab30459f02) debugging travis-ci issue with java version
- [2332098](https://github.com/jspreddy/dql/commit/23320989e02eb2d6f8a4fb148fedb15c2f8d36bd) More dev notes on how to test, install package, run executable
- [2825925](https://github.com/jspreddy/dql/commit/2825925b225eec7871296fa936e0b4111e290309) Enabling auto-commit on bump2version
- [b8194ff](https://github.com/jspreddy/dql/commit/b8194ff6bbfdaea97c23910c61f979d1b3594d26) Using bump2version instead of bumpversion, updating docs with the info
- [6e8b133](https://github.com/jspreddy/dql/commit/6e8b133a9e96d7c9614596c1f382dd12d20d6ea9) Ignoring .idea folder
- [51ac0c2](https://github.com/jspreddy/dql/commit/51ac0c282bc63db2f66daacef7e05a976982bab6) Updating the docs for developers
- [8658f5f](https://github.com/jspreddy/dql/commit/8658f5f595cec77dacd49e6fe07bf180434ccf32) recomending java installations
- [3f08d8e](https://github.com/jspreddy/dql/commit/3f08d8e2a59fe32ab22e15dbd174df0f0f658774) python version removal
- [9f98e94](https://github.com/jspreddy/dql/commit/9f98e944d2b819b03ff839da005832b61cd473a1) adding tox and tox-pyenv to the dev requirements + docs for dev
- [44e2fb8](https://github.com/jspreddy/dql/commit/44e2fb875ebb4cd9dc97c60044997ddcf635f68f) Getting project to work with pyenv
- [265a3ab](https://github.com/jspreddy/dql/commit/265a3ab548e27206c60f6a40d9de3fe20fc1089c) Typo
- [192325b](https://github.com/jspreddy/dql/commit/192325ba7c95418ddf94ba540ae578886ae1dd06) Remove python 2 code from the install.py file
- [59fe0eb](https://github.com/jspreddy/dql/commit/59fe0eb2b6cfe3cbe584d2eba3fd3393838d0abc) Remove python 2-to-3 compatibility code
- [fba9d85](https://github.com/jspreddy/dql/commit/fba9d85225d48c03d685f9bc276cb852b2cf7b5e) Drop a python-dateutil version constraint that we no longer need
- [7c90ef5](https://github.com/jspreddy/dql/commit/7c90ef5b573e474cc2c99014eff8cfafaf3802de) Drop official support for python 2
- [7b1830d](https://github.com/jspreddy/dql/commit/7b1830dabb10375e2fd62212e26761ace5c63b31) Reformat files with black and isort
- [161db92](https://github.com/jspreddy/dql/commit/161db929921f44b17ee64b05a9c55507e6a8b83a) Use isort with black, and provide a tox command for autoformatting
- [251f616](https://github.com/jspreddy/dql/commit/251f616f78efb4a057f7b4ad70f9655ab284bba3) Remove run_autopep8.sh script

## dql (Python) 0.5.28 (2019-11-20)

Contributors: Steven Arcangeli

- [66e5f74](https://github.com/jspreddy/dql/commit/66e5f74fd1d300131a8321d75d0fd3d5ea6fbc1b) Update changelog
- [614ee9e](https://github.com/jspreddy/dql/commit/614ee9ed282515481661e8f11206841493044dfd) Add support and tests for python 3.7
- [50fef38](https://github.com/jspreddy/dql/commit/50fef38b6b56e2c316bafc5628bab5eda17ad2b3) Fix #19: Can't SAVE to a format

## dql (Python) 0.5.27 (2019-10-26)

Contributors: Steven Arcangeli

- [c146ae3](https://github.com/jspreddy/dql/commit/c146ae3b114e3acfe41c9c5e04c17a24d63a91c5) Updating changelog
- [c950a0d](https://github.com/jspreddy/dql/commit/c950a0d5e798f59c76f310321e4c07444d3270cb) Drop support for python 3.4
- [38738e9](https://github.com/jspreddy/dql/commit/38738e9c5f25af7033d1528aaaff9fa3bcdd6c9e) Black
- [a69f74f](https://github.com/jspreddy/dql/commit/a69f74fcd0e03d441f905540e9e454f54de06827) throttle: fix Python 3 compatibility
- [a29cab9](https://github.com/jspreddy/dql/commit/a29cab9834d2265360c40327300489c97c47cc9d) Format everything with black
- [4458e44](https://github.com/jspreddy/dql/commit/4458e44f2aff162b8947f650e8fcad548fb46193) Mention tox in docs on developing
- [f720860](https://github.com/jspreddy/dql/commit/f7208607659ff53d8e53943e574915a49c3ad4b6) Fix linter errors
- [4881c2c](https://github.com/jspreddy/dql/commit/4881c2cbcb42685a13cd0d6a9a9950aa00ad6913) Fix #20: Update docs for developing
- [e9d3aa2](https://github.com/jspreddy/dql/commit/e9d3aa22873076dae5ebd02e35318aa996b1e56a) Format everything with black
- [1adf345](https://github.com/jspreddy/dql/commit/1adf345ef34d1acc79231d06ef4fd7540f5416a1) Mention tox in docs on developing
- [81d3f6f](https://github.com/jspreddy/dql/commit/81d3f6f2a0c2f841a937462f2e4737bbce9e6207) Fix linter errors
- [b3e2611](https://github.com/jspreddy/dql/commit/b3e2611bfbf248a847c42b8dbd3420046d3ce852) Fix #20: Update docs for developing

## dql (Python) 0.5.26 (2018-03-17)

Contributors: Steven Arcangeli

- [63ae210](https://github.com/jspreddy/dql/commit/63ae2108c7057f47b22a41896a10cf6795fac630) Updating changelog
- [c7b5359](https://github.com/jspreddy/dql/commit/c7b535957731a94a7f5c58e22ff4c7f99a403995) Fix requirements conflict with botocore
- [374af3e](https://github.com/jspreddy/dql/commit/374af3edbf449b87617b1c571ce21c43a368c683) Migrate from six to python-future
- [cc16bf4](https://github.com/jspreddy/dql/commit/cc16bf4875798c73100943b92a22bd0f7a1e01e6) Package as wheel

## dql (Python) 0.5.25 (2017-12-10)

Contributors: Steven Arcangeli

- [16ab0a8](https://github.com/jspreddy/dql/commit/16ab0a868095ccfb8bf5ef564a9e565903cbcefb) Update changelog
- [76c77fb](https://github.com/jspreddy/dql/commit/76c77fbaa856243ed7aebf5d2500ba254a6b365b) Update linter and fix lint
- [83b4223](https://github.com/jspreddy/dql/commit/83b4223418fd4df2d21a76ceafc08eaa0efa2364) Removing dependency on future
- [188cd85](https://github.com/jspreddy/dql/commit/188cd854bcb2584f3ce8ba92bbe2b7deb3facd20) Fix DQL to work with Python 3

## dql (Python) 0.5.24 (2017-09-12)

Contributors: Steven Arcangeli

- [fb9e3dc](https://github.com/jspreddy/dql/commit/fb9e3dc63b2beb9fbac688dc101eec577e4181c4) Updating changelog
- [afc1830](https://github.com/jspreddy/dql/commit/afc1830571207c56d4a8518df79920734ada1f18) Update test config to run on python 3.6
- [705f8ad](https://github.com/jspreddy/dql/commit/705f8adb5a900939445ce5c564672241b68bd69e) Remove old crap
- [110cc04](https://github.com/jspreddy/dql/commit/110cc04f0a6d780c82a059b1a3b38eeab49d1b65) Fix #12: Problem with dashes in the key names

## dql (Python) 0.5.23 (2017-07-25)

Contributors: Steven Arcangeli

- [6ef3f18](https://github.com/jspreddy/dql/commit/6ef3f18eac849aa1b05ddd19e8836442114878d3) Updating changelog
- [23e5d32](https://github.com/jspreddy/dql/commit/23e5d323d916211ea0b166fd8c44c0b66f47318d) Dropping support for python 2.6
- [071acf2](https://github.com/jspreddy/dql/commit/071acf2377ba4742f8d07efcc0d654b179d964c1) Fix whitespace
- [2352c61](https://github.com/jspreddy/dql/commit/2352c61910c1d1fa407d067e4d0c677d1836330e) Upgrading pylint
- [4e16922](https://github.com/jspreddy/dql/commit/4e169222129e89c2c05f359b24e4d5260a093eff) Fix default encoding bug on mac
- [8043d40](https://github.com/jspreddy/dql/commit/8043d4062f501e011161ffd201630d350e29d632) Specify min version of six
- [811c988](https://github.com/jspreddy/dql/commit/811c988f015e437bc49232411adad05ca707126a) Don't use containers for travis tests
- [14f5cb0](https://github.com/jspreddy/dql/commit/14f5cb0363368f1c5d658efd5df213d25b2a078d) Tests run on python 3.5
- [d602a45](https://github.com/jspreddy/dql/commit/d602a451fd011534897cda8bf8ba1ed9bbbc14ed) Tox tests are verbose

## dql (Python) 0.5.22 (2016-10-04)

Contributors: Steven Arcangeli

- [4248517](https://github.com/jspreddy/dql/commit/424851777412c65f917c03bdcdb8bba754ed98e6) Updating changelog
- [1cf3b4f](https://github.com/jspreddy/dql/commit/1cf3b4fa366532e635cc2bcb465618a8bb409483) Fix #6: Error running non-dql statements with -c
- [795722e](https://github.com/jspreddy/dql/commit/795722e002a8c48773d7704cbddd868447a039a2) Update repo path

## dql (Python) 0.5.21 (2016-09-06)

Contributors: Steven Arcangeli

- [46c1f94](https://github.com/jspreddy/dql/commit/46c1f945bcc7a5907806a96e4736fef6aeefd48f) Updating changelog
- [4b021d9](https://github.com/jspreddy/dql/commit/4b021d9cd34ff04fe0fd4cbf05c3addfbad8cc04) Dynamically size the watch columns better
- [95d718b](https://github.com/jspreddy/dql/commit/95d718be47b4d270752255e18074b3b6e2f28ccf) Watch results are sorted
- [bad3c44](https://github.com/jspreddy/dql/commit/bad3c44060137c4faa9edc5bd960b963aee1b65c) Fix a crash resizing window when running 'watch'

## dql (Python) 0.5.20 (2016-07-28)

Contributors: Steven Arcangeli

- [e939244](https://github.com/jspreddy/dql/commit/e9392449ab2f507b14d0fc24fa02037b946a6229) Updating changelog
- [cb563f1](https://github.com/jspreddy/dql/commit/cb563f18d508a8db580d1898c05d10f74377e51b) Fix a name replacement bug with IN filters
- [fc2f02e](https://github.com/jspreddy/dql/commit/fc2f02eb9cf796ab55ac4c71f0358f5529ee1b3d) Updating readme to mention the install script
- [4fe72de](https://github.com/jspreddy/dql/commit/4fe72def959b30175398d60eda4d95b5e69bb61b) Add a --version argument
- [c3e375e](https://github.com/jspreddy/dql/commit/c3e375efff6289b4d0a24e654f51b116d6d5fc75) Fix JSON serialization when saving to file

## dql (Python) 0.5.19 (2016-06-15)

Contributors: Steven Arcangeli

- [2d40720](https://github.com/jspreddy/dql/commit/2d40720afb2a2b0281e3698895e89af765f73249) Updating changelog
- [8d20710](https://github.com/jspreddy/dql/commit/8d20710795c267a83d087d13592c560c2ad10d15) Lock in the version of pyparsing

## dql (Python) 0.5.18 (2016-05-18)

Contributors: Steven Arcangeli

- [a076ddf](https://github.com/jspreddy/dql/commit/a076ddf6f09b432f183aef864347b47fde55b3c7) Updating bumpversion config
- [b246841](https://github.com/jspreddy/dql/commit/b2468412b4fc5a9931a446a54ad8f9fcb761c896) Updating changelog
- [ac2a3a9](https://github.com/jspreddy/dql/commit/ac2a3a9f8bac66f2655bd5af783bc558055e9c66) Switching out the standalone runner for a better version
- [a931c2c](https://github.com/jspreddy/dql/commit/a931c2c9e18e48a83d189d22e1251ab09cda0164) Fix some selection logic
- [e30a4f0](https://github.com/jspreddy/dql/commit/e30a4f0ff3333b6d8429e6b6c1562a2d3a99d816) Correctly perform path substitution in expressions
- [cd3b428](https://github.com/jspreddy/dql/commit/cd3b428b2e3d1a58d7995569d0a82a5a06e54493) Pre-commit hook for checking README format

## dql (Python) 0.5.17 (2016-05-06)

Contributors: Garrett Heel, Steven Arcangeli

- [d2f5824](https://github.com/jspreddy/dql/commit/d2f5824826065833bf7cfa87ade4cae81ad82ff0) Updating changelog
- [5b916ae](https://github.com/jspreddy/dql/commit/5b916aec79e8f7c715dd0ddc0d2f73913a2ee47c) Fix requirements_dev.txt
- [52a3a70](https://github.com/jspreddy/dql/commit/52a3a70cd99e7d7340b44b05c6c0e628effaccdc) Better support for displaying/saving binary types
- [86c8c63](https://github.com/jspreddy/dql/commit/86c8c63060153e47031e2e395c79fdd18b5ea6c2) Fix more bad docs for USING
- [8abcc72](https://github.com/jspreddy/dql/commit/8abcc72ea4a1a53edeb5781a99629f5e0af6cff4) Fix incorrect grammar for USING in doc examples (#5)

## dql (Python) 0.5.16 (2016-03-05)

Contributors: Steven Arcangeli

- [de50efc](https://github.com/jspreddy/dql/commit/de50efc95945b78a3c02f1ed6efe4b6203c0f250) Updating changelog
- [d916530](https://github.com/jspreddy/dql/commit/d916530a137bcc7377d00827908429d6d92158d3) Fix bug with using inline booleans in statements

## dql (Python) 0.5.15 (2016-02-16)

Contributors: Steven Arcangeli

- [060e969](https://github.com/jspreddy/dql/commit/060e96954ee5134d1e2451327223722fb8f0ff83) Updating changelog
- [dc65c34](https://github.com/jspreddy/dql/commit/dc65c34a062c7471a991be8b220ab5f29db3714f) Handle missing libs for windows

## dql (Python) 0.5.14 (2016-02-11)

Contributors: Steven Arcangeli

- [8f74d1d](https://github.com/jspreddy/dql/commit/8f74d1d48b5565074ccd1aba7675b0f9ce511a55) Updating changelog
- [1db2135](https://github.com/jspreddy/dql/commit/1db2135798e72a5b433799edbfb39c8289fa6ef0) Missing curses doesn't raise ImportError

## dql (Python) 0.5.13 (2016-02-10)

Contributors: Steven Arcangeli

- [e71c5ad](https://github.com/jspreddy/dql/commit/e71c5adba6c4cab2a775e9afe21ef6e1028ccb7d) Updating changelog
- [01cb758](https://github.com/jspreddy/dql/commit/01cb758c7be18b91f6ef25722e8551f13a589a38) Fix 'No Results' bug

## dql (Python) 0.5.12 (2016-02-09)

Contributors: Steven Arcangeli

- [67d98e8](https://github.com/jspreddy/dql/commit/67d98e8bb2d800c6a2cfb8ca29b254debb4b5d4d) Update changelog
- [13a7578](https://github.com/jspreddy/dql/commit/13a75785bc2ea074bdc01eef1012923e82ac7d8b) Pylint
- [ad91f92](https://github.com/jspreddy/dql/commit/ad91f928fb728ac7c78a0d4c824acb360bd14012) Fixing a test
- [be60f67](https://github.com/jspreddy/dql/commit/be60f67988126f913ab6d971cf669f91332c2734) Fix ORDER BY when using 'SELECT field AS alias' syntax
- [7746d48](https://github.com/jspreddy/dql/commit/7746d48854f0e4b6f5ad267453cb22c9e9fea6b0) Warn before running delete/update on an entire table
- [da33c73](https://github.com/jspreddy/dql/commit/da33c7334426420a0708b2b41689a87abf0722d6) Document the new throttling capabilities
- [8eeec16](https://github.com/jspreddy/dql/commit/8eeec167724a874b0d231661e47cd4c1799ce9fa) Controls for throughput throttling
- [714e326](https://github.com/jspreddy/dql/commit/714e326e3ac4185178d154b8e9aed99e2af129b4) Better error messages when querying a nonexistent table
- [d94077e](https://github.com/jspreddy/dql/commit/d94077e5b09eef778bc05c4a3a8e458b8f9ab3c1) Don't prompt user to continue if no results are left
- [2585f09](https://github.com/jspreddy/dql/commit/2585f09ee1f765f6a7f2ed489def8eeb61109121) Fix compatibility with pyparsing 2.1
- [fc7222e](https://github.com/jspreddy/dql/commit/fc7222e1e9b1787e1093a201a5e3bbe3e41a0d42) Don't require filenames with slashes to be quoted
- [63b2ac5](https://github.com/jspreddy/dql/commit/63b2ac5c1244e68859cfd6ca73907188f777a880) More robust venv validity checking in run_dql.py
- [e3777ed](https://github.com/jspreddy/dql/commit/e3777ed6e1e72d8c796f9a2f2c6c01b711747e6e) Don't crash when selecting TIMESTAMP(NULL)
- [3625e59](https://github.com/jspreddy/dql/commit/3625e597dd34b2dc0990f2d0d167a02771582006) Change behavior of LIMIT and add SCAN LIMIT

## dql (Python) 0.5.11 (2015-12-10)

Contributors: Steven Arcangeli

- [74bd7f6](https://github.com/jspreddy/dql/commit/74bd7f65276462a2944d87d1f107dbd6a016b6ea) Updating docs and changelog
- [d6b7c8e](https://github.com/jspreddy/dql/commit/d6b7c8e153b6f1f4bde843ab316ec6c9d8fd34bf) SELECT <attributes> can now use full expressions
- [8380d7b](https://github.com/jspreddy/dql/commit/8380d7bbea7df7fc87eb60f7a59d28508141ec9a) Refactor and split up expressions
- [4f8fd3f](https://github.com/jspreddy/dql/commit/4f8fd3fbd80a5420b9510a41e1bc408690c0447c) Set output format changed to '(1, 2, 3)'
- [682cd83](https://github.com/jspreddy/dql/commit/682cd83f8d631131f1ee089883a748c4ff7890ea) Update options docs

## dql (Python) 0.5.10 (2015-12-09)

Contributors: Steven Arcangeli

- [1d5a4aa](https://github.com/jspreddy/dql/commit/1d5a4aa771498ba0aad61084360de3cb5e8450f9) Twiddle a test for floating error
- [26be140](https://github.com/jspreddy/dql/commit/26be1404508baa2d01bdfd634b0095fc34f0e73e) Remove bad test
- [8430f35](https://github.com/jspreddy/dql/commit/8430f358e3635115728e75b2fdedcc468f47bc2c) Updating changelog
- [b30f520](https://github.com/jspreddy/dql/commit/b30f520290c1ae18f26f7d25f2741ab4c5108904) LOAD command and SAVE improvements
- [d29bcd0](https://github.com/jspreddy/dql/commit/d29bcd03cf0aad432df819975471d4c61c905cbc) Add basic timestamp functionality
- [3d3fae9](https://github.com/jspreddy/dql/commit/3d3fae906271a8dd2531e59c03c2b0b5db74ed34) Fix a paging bug

## dql (Python) 0.5.9 (2015-12-06)

Contributors: Steven Arcangeli

- [a601af5](https://github.com/jspreddy/dql/commit/a601af551fb850bc8551d5142918bcf645dbf9d7) Updating changelog
- [3640d9a](https://github.com/jspreddy/dql/commit/3640d9a126be314e1ace1a7006cd84e0cd0d83b2) Fix bad formatting when no results found
- [055bdd3](https://github.com/jspreddy/dql/commit/055bdd363722df59c36beb5ce1cc6de27fe9bac4) Rework and improve the formatters
- [8853e3f](https://github.com/jspreddy/dql/commit/8853e3f01998256b5b4aa8cc9e26685e3ca5ec1a) Fix infinite loop in ORDER BY results
- [7f70248](https://github.com/jspreddy/dql/commit/7f70248726b6bc15f5e125539a1d714ef9ea2852) Select only specific attributes in KEYS IN query
- [3252e02](https://github.com/jspreddy/dql/commit/3252e02c3ec45440fba0c8d9028280fc92fdf14a) Auto set pagesize from terminal height
- [26cfa12](https://github.com/jspreddy/dql/commit/26cfa12cb7b6aa8f54992a379f6d1160e363f4db) Don't print out results when saving to a file

## dql (Python) 0.5.8 (2015-12-01)

Contributors: Steven Arcangeli

- [0c243c6](https://github.com/jspreddy/dql/commit/0c243c684829359e7b2131b30d1bdfe26495b098) Tab completion for Mac OS X

## dql (Python) 0.5.7 (2015-12-01)

Contributors: Steven Arcangeli

- [cae44ab](https://github.com/jspreddy/dql/commit/cae44ab4e444a22ff5e9c5e1a5b93e3d89fedda5) Updating changelog
- [57558e2](https://github.com/jspreddy/dql/commit/57558e2016cb893764b3ac054b2a07d5e5971354) Auto-detect terminal width for output
- [6e24b25](https://github.com/jspreddy/dql/commit/6e24b2557d9f126eb8f514e3b999ecc6b19f440d) The run_dql.py script locks in a specific version

## dql (Python) 0.5.6 (2015-12-01)

Contributors: Steven Arcangeli

- [7115c09](https://github.com/jspreddy/dql/commit/7115c09c696b9fd0725a7031ea88c26fcfb6751f) Update changelog
- [a343117](https://github.com/jspreddy/dql/commit/a3431178ef0ffc03072817bd03eb45df4191dc08) Always batch_get after querying/scanning non-all-projected index
- [edc83d3](https://github.com/jspreddy/dql/commit/edc83d3f5b63d54bbb25c90bc345a4a3de2c2785) Load saved formatter option on startup
- [6f3ed20](https://github.com/jspreddy/dql/commit/6f3ed203fd606a0c62781d0c6e78b52c7997e0ed) Make tests pass
- [f88e988](https://github.com/jspreddy/dql/commit/f88e9887faf03ca63438ba570b14964e02fa93ec) Allow comparing fields in filter expressions

## dql (Python) 0.5.5 (2015-11-25)

Contributors: Steven Arcangeli

- [b5f5a75](https://github.com/jspreddy/dql/commit/b5f5a7579e0155d13268c157bd0d4ec24426b04e) Updating changelog
- [3cdf4f6](https://github.com/jspreddy/dql/commit/3cdf4f6b3c4feca912a062c079112648442b582e) Better support for -c arg
- [925ad3d](https://github.com/jspreddy/dql/commit/925ad3d419e99ed0ea4ce058641aafa3bc5365c8) Helpful self-contained dql.py executable
- [2958dca](https://github.com/jspreddy/dql/commit/2958dca2389cfe703d991ce5b3ed1757a52d324c) Prevent a cryptic error message when doing a batch_get post-select
- [11bd6a2](https://github.com/jspreddy/dql/commit/11bd6a2f6bcf63cd7422cc40486fc480cca9c55b) Clarify documentation of LIMIT
- [ebeee6b](https://github.com/jspreddy/dql/commit/ebeee6bf0fa6d0fb3654f7a3525bc969b10b72b7) Fix some bugs around SELECT...SAVE

## dql (Python) 0.5.4 (2015-11-18)

Contributors: Steven Arcangeli

- [42d7258](https://github.com/jspreddy/dql/commit/42d7258e6e707a8ce0db1cd1d7d51d03d013cc3b) Updating changelog
- [1428f93](https://github.com/jspreddy/dql/commit/1428f93bf61fb648dceaa3a3fbe1f2142ac82313) Curses monitor responds to terminal resize
- [63d64a7](https://github.com/jspreddy/dql/commit/63d64a7f54f04176508109e4f9fc3221f98452f4) Fix bug in watch display
- [5492ad9](https://github.com/jspreddy/dql/commit/5492ad9a55764a2eea0f3c42fef1ef303aa68501) Can save query output to a file

## dql (Python) 0.5.3 (2015-11-17)

Contributors: Steven Arcangeli

- [3d1c0dc](https://github.com/jspreddy/dql/commit/3d1c0dc6038f9811c66bdca26561a7b55d5fab34) Updating changelog
- [b51f20c](https://github.com/jspreddy/dql/commit/b51f20cf89ab03c76af8316debf6613bc41cd3f9) PEP8
- [6886719](https://github.com/jspreddy/dql/commit/6886719dc981a52f7efc39dcb658162c815c9164) Fix test for new version of pyparsing
- [7ff53a6](https://github.com/jspreddy/dql/commit/7ff53a659e94efeb1f60c8d9d0eb8273e59c34cb) Add 'IF (NOT) EXISTS' to ALTER commands
- [d7b256d](https://github.com/jspreddy/dql/commit/d7b256de6289beae3ac3f212c7d8178fc8e37311) Allow select attributes that aren't projected into the queried index
- [acab66e](https://github.com/jspreddy/dql/commit/acab66e6b18b3b9389bc73e547a9fe627498dec1) Curses display that monitors consumed capacity of tables
- [d9181d4](https://github.com/jspreddy/dql/commit/d9181d4031de1891bffc6def0a50e28b095f7d44) Add an ORDER BY to SELECT

## dql (Python) 0.5.2 (2015-11-13)

Contributors: Steven Arcangeli

- [a166473](https://github.com/jspreddy/dql/commit/a1664736bcf61cb8e2bb102827161adcc10edb47) Remove debug print statement
- [dee34e8](https://github.com/jspreddy/dql/commit/dee34e8c11279231394ae63d5ca2728761756945) Updating changelog
- [3c0be82](https://github.com/jspreddy/dql/commit/3c0be821a2a76e4917f80cd1e35dcfbca839bea3) Add ANALYZE query and simplify EXPLAIN logic
- [234cde6](https://github.com/jspreddy/dql/commit/234cde6753ec50b57cd7509aaa9789732fe3bf62) Change the ls formatting
- [874dc3f](https://github.com/jspreddy/dql/commit/874dc3fa881d78da2fdc8235924d0ee12bff7636) Add an EXPLAIN query

## dql (Python) 0.5.1 (2015-11-10)

Contributors: Steven Arcangeli

- [4b7c2dd](https://github.com/jspreddy/dql/commit/4b7c2dd23d665c128ec3e011f5c02f38c0f0be15) Pretty-format SELECT count(*)
- [8271cf8](https://github.com/jspreddy/dql/commit/8271cf8d0078f61bc7eb25bc326ed3b92b820b1f) Return raw values from query, pretty-format for CLI
- [5c980ff](https://github.com/jspreddy/dql/commit/5c980ff6555be4b8846d1c40f8cd2984405f2c36) Disallow passing credentials on command line

## dql (Python) 0.5.0 (2015-11-10)

Contributors: Steven Arcangeli

- [803e44f](https://github.com/jspreddy/dql/commit/803e44f7b242a5fe1ae7d5a3185bd4a860523b3e) Fix tests in python 3
- [cbf3dc9](https://github.com/jspreddy/dql/commit/cbf3dc942f171ce22a2f664f10229e2ad893ca21) Fix pylint error
- [b480909](https://github.com/jspreddy/dql/commit/b480909564369566213767f758051dfb3ccb8dd5) Update README
- [862c792](https://github.com/jspreddy/dql/commit/862c7924331a9d1b52ea3b5c86c4ba633ed19f95) Rewrite DQL for new DynamoDB expression API

## dql (Python) 0.4.1 (2015-05-15)

Contributors: Steven Arcangeli

- [70d2068](https://github.com/jspreddy/dql/commit/70d20688855826a063658e36eff34441f206faeb) Updating changelog
- [065552d](https://github.com/jspreddy/dql/commit/065552d474d5f6badd55cb12c1971fdf81c37922) Fixing some pylint issues
- [56fff86](https://github.com/jspreddy/dql/commit/56fff86b3a8f32f03c26149c8f3d8f39a037dd90) Update for dynamo3 0.4.0 and improve CloudWatch handling

## dql (Python) 0.4.0 (2014-10-13)

Contributors: Steven Arcangeli

- [f8adceb](https://github.com/jspreddy/dql/commit/f8adceb33ba87415be20c935b6cf8880ef062041) Fixing tests for python 3
- [f138144](https://github.com/jspreddy/dql/commit/f138144e90d0ed7301c6735a9d562ff2ec90ffbc) Adding support for JSON data types
- [9a783f7](https://github.com/jspreddy/dql/commit/9a783f773a5fab6e52da08937d04c99ba9043268) Change how DQL connects to DynamoDB Local
- [aa49085](https://github.com/jspreddy/dql/commit/aa49085c5f5d2b3052d34e1434366ad1338b1982) Change default page size to 100
- [00b6c50](https://github.com/jspreddy/dql/commit/00b6c50f88ade31bc00512d0666bf7c3adb7c8bf) Pretty-format json data when in expanded view

## dql (Python) 0.3.2 (2014-08-26)

Contributors: Steven Arcangeli

- [b0730c5](https://github.com/jspreddy/dql/commit/b0730c5a1a3427427bad6422464b7a5295655d1c) Correct prompt text when connected to local DynamoDB
- [ed88d31](https://github.com/jspreddy/dql/commit/ed88d31e48a5585be6714830e4171dae627aafb1) Dump schema command allows '.' in table name
- [881bfb7](https://github.com/jspreddy/dql/commit/881bfb7fa7e1f4204abf564b3def88b7540604e3) Parse port argument as an int

## dql (Python) 0.3.1 (2014-08-20)

Contributors: Steven Arcangeli

- [7dfcb0d](https://github.com/jspreddy/dql/commit/7dfcb0d1ecda8c15d9e959acd4fb31ef796deb9f) Metadata cleanup
- [d55d036](https://github.com/jspreddy/dql/commit/d55d0369c216c40bac30491ffde1233ba272975e) Update build links in README
- [4843f35](https://github.com/jspreddy/dql/commit/4843f3557bf4ced111e80e6e9f6e16b90ce47135) Updating changelog
- [39c39aa](https://github.com/jspreddy/dql/commit/39c39aa498b41c18348b792e5ba8ebc50cfec562) Allow '.' in table names

## dql (Python) 0.3.0 (2014-06-27)

Contributors: Steven Arcangeli

- [ce80e4f](https://github.com/jspreddy/dql/commit/ce80e4fb5215c6fdd6b753b85f54c23d7cd8e175) Updating changelog
- [4be2679](https://github.com/jspreddy/dql/commit/4be26798d3de890d608abeac2012bfc8b303fe75) Adding new dynamo FILTER features

## dql (Python) 0.2.1 (2014-05-12)

Contributors: Steven Arcangeli

- [318b586](https://github.com/jspreddy/dql/commit/318b586c1e6ee08a124f6f16a715306856e881b5) Fixing some pylint warnings
- [056c426](https://github.com/jspreddy/dql/commit/056c4265745228cb46f255a0caa3ffcf299e8ef2) Updating changelog
- [1efc67e](https://github.com/jspreddy/dql/commit/1efc67eb594b543d47e89062a9b05f1d9e9cebf0) Print integer results from engine (closes #1)
- [6692a35](https://github.com/jspreddy/dql/commit/6692a355146f3a005d78bd84afc94774c5d61aea) Updating documentation
- [a4904f0](https://github.com/jspreddy/dql/commit/a4904f033864888fd466dcc062d3d3ad93a0c3e3) Use bumpversion to manage version number
- [9310efc](https://github.com/jspreddy/dql/commit/9310efc457226543002b69006fadb1705365df99) Completed Python 3 support
- [c0ab620](https://github.com/jspreddy/dql/commit/c0ab62014c5c97961100f277cb6f8f2203dbacb0) Everything but cloudwatch works with python 3
- [42dd372](https://github.com/jspreddy/dql/commit/42dd3726297b86136cf7bfa9b8b1045975d1dd9a) Experimental python3 support
- [bdf8b72](https://github.com/jspreddy/dql/commit/bdf8b72dae684515a607cde5fc08df460f3d7399) Documentation improvements
- [adaccee](https://github.com/jspreddy/dql/commit/adaccee6ce7570675afa377ff392d921b10e3318) Setting up for release
- [0b28787](https://github.com/jspreddy/dql/commit/0b28787350a10e9f6f254529a762d8c03e35fa6a) Adding more tests
- [1d97bdd](https://github.com/jspreddy/dql/commit/1d97bdde69cac1fcccd78cdc0eb019fc54fc8461) Adding nose and mock to setup requirements
- [52d5bd1](https://github.com/jspreddy/dql/commit/52d5bd1073fc9f7a47d3d3a15982a68d85ba82ad) Engine scope is persistent, not part of execute()
- [d6d6c7f](https://github.com/jspreddy/dql/commit/d6d6c7f74297c5773f98c1b1ff6be91f6e589e0f) Don't truncate fields in expanded format
- [3a9531b](https://github.com/jspreddy/dql/commit/3a9531ba09e723bbad9385cb7fbe5dc4be706d67) DQL will update throughput in pieces if necessary
- [a13e978](https://github.com/jspreddy/dql/commit/a13e9782f1428f2f45c47ad8a0baec75a6dcaf7d) Smart formatting of query results
- [1e174b9](https://github.com/jspreddy/dql/commit/1e174b9c3826c90edb624ab9a55353f32450d761) Improving the output format
- [2541e08](https://github.com/jspreddy/dql/commit/2541e0867e7673e25045fb38194533954f9ca3a6) Increasing test coverage
- [5059bef](https://github.com/jspreddy/dql/commit/5059bef514df8f6b5df0a13f3c6973c68d176853) Fixing crash when describing a table being deleted
- [6e6c968](https://github.com/jspreddy/dql/commit/6e6c968bd009562275888805317e2dab499c9247) Moving dev documentation from readme to docs
- [1a2c05a](https://github.com/jspreddy/dql/commit/1a2c05a380f370d0e0e2a5b676d8e7f87971ce7c) Better tab completion for table names
- [ac6a1df](https://github.com/jspreddy/dql/commit/ac6a1df1149d71297165a285f0a393e5f0ab1238) Updating language to support different index projections
- [61a639f](https://github.com/jspreddy/dql/commit/61a639f79ca8b0ad525a49183bb0825c6e811b9d) Can autoload python commands from conf file
- [908b070](https://github.com/jspreddy/dql/commit/908b070d57062cc3668fa3f0054ee3c10935d5f0) Updating documentation
- [210fc1f](https://github.com/jspreddy/dql/commit/210fc1f2923e864fdc30c189a1e4ff5ec2f9c12c) Allow more complex python expressions
- [6024665](https://github.com/jspreddy/dql/commit/6024665b8bbf3bc081d0503083dc60ebb0b03629) Save format and display options to config file
- [c8e4611](https://github.com/jspreddy/dql/commit/c8e46110afe2fc60362f02aa9372261deaa3832b) Adding more output format options
- [aec78f0](https://github.com/jspreddy/dql/commit/aec78f09a791657d1f0ea26194737541afde6a55) Adding unittest2 as tox dep
- [e43c911](https://github.com/jspreddy/dql/commit/e43c9113c077968537a38abec97c46f71bb1d124) Making sure we use unittest2 for tests
- [dbfecd8](https://github.com/jspreddy/dql/commit/dbfecd8b955e9bff6af8c96bbfc9399353f64d7c) Fixing tox deps for python 2.6
- [4c25764](https://github.com/jspreddy/dql/commit/4c257648637cd3125dffe90c5557fa654f5b5065) Adding support for atomic add/remove set operations
- [e8502cd](https://github.com/jspreddy/dql/commit/e8502cd24e17015996e85c01b5f4c8200a13737a) Updating docs and moving tests outside package
- [185e630](https://github.com/jspreddy/dql/commit/185e63035fe75d12bd91645fa1ddfb279d3ddb49) Renaming version helper to avoid name collisions
- [be4d95f](https://github.com/jspreddy/dql/commit/be4d95f1d8fa7045f89c838e1285e9e868f2901e) Making delete queries more efficient
- [4a739b0](https://github.com/jspreddy/dql/commit/4a739b06bff45888e6c1626bb4dd605ab0333b3a) Fixing a formatting error in engine.py
- [40ae18a](https://github.com/jspreddy/dql/commit/40ae18a83215ee25d7d92243aa29e8b58b06887b) Updating pylint config
- [a95747a](https://github.com/jspreddy/dql/commit/a95747a9fc1c2d694fa8ad214ab44c5d6e963dac) New docs format
- [e98344e](https://github.com/jspreddy/dql/commit/e98344e06df20fb14e5c56852ee94dce3f446401) Removing unused code
- [cda0c67](https://github.com/jspreddy/dql/commit/cda0c673575353c79b2188dafa921e28b7d87f4e) Nose plugin silences dynamodb local output
- [6d020d0](https://github.com/jspreddy/dql/commit/6d020d0d38b982096c9a37327bcc0daba231d909) Attempting to fix tests on python 2.6
- [690ffa7](https://github.com/jspreddy/dql/commit/690ffa77a2fbb85dd1f4d687202662d2ed684a43) Attempting to fix build on python 2.6
- [04dd78e](https://github.com/jspreddy/dql/commit/04dd78eae2a5e45173934062083e3b3f747b3887) Updating nose plugin and docs for new DynamoDB Local
- [efee2f6](https://github.com/jspreddy/dql/commit/efee2f60dbba3af67d0b7645a18ab6c49e224154) Correcting typo
- [7ad9267](https://github.com/jspreddy/dql/commit/7ad92670085276d483a8b21caf2b0dba2769df36) Correcting typo
- [545f624](https://github.com/jspreddy/dql/commit/545f62418d1c90179e6a7b37fe73f18184a16a63) Adding support for global secondary indexes
- [9b7f5a6](https://github.com/jspreddy/dql/commit/9b7f5a6940e20db094e01216de953f838aa8fd89) Changing create table syntax
- [1fffe97](https://github.com/jspreddy/dql/commit/1fffe977aae48b1f0d39b9dc10498243f8d38963) Can alter just read or just write throughput
- [51e9e67](https://github.com/jspreddy/dql/commit/51e9e67297c244c56f80ac87ce48638ba10b2bff) Update boto requirements to fix DynamoDB api
- [31ac082](https://github.com/jspreddy/dql/commit/31ac082505db5edf9e3757f6dc5fcc8e555d4fd4) Engine has a method for directly connecting to a region
- [b09ff78](https://github.com/jspreddy/dql/commit/b09ff787ea6c809a70c93ca965749ddd483a08ed) Default region can be set in environment variable
- [6e9567e](https://github.com/jspreddy/dql/commit/6e9567e7afed813f42e9786382f6a97bfd86ee46) Further monkey patches around boto
- [9151d75](https://github.com/jspreddy/dql/commit/9151d758570b6c4d176a70407cc6919bddeda065) Hacking a way around float rounding errors in boto
- [8f0a0c1](https://github.com/jspreddy/dql/commit/8f0a0c18cb6d037439469555eb701ce03a627233) Removing some unused functionality
- [d324ec7](https://github.com/jspreddy/dql/commit/d324ec72d88b866f4c1dd583e5826ca5d0f44f1d) Adding a way to run DQL inline with python
- [9ef7fb3](https://github.com/jspreddy/dql/commit/9ef7fb333ab240bb056d5662e92d6e306677ecdd) Engine has a scope and can do variable lookup
- [cce078d](https://github.com/jspreddy/dql/commit/cce078da4ee19a6d3c335a178c0d888e5b78e29f) Adding autocomplete for ls
- [e572775](https://github.com/jspreddy/dql/commit/e5727757c68ec921f9f20db52fbf2462d0504693) Table description includes CloudWatch metrics
- [5d5c941](https://github.com/jspreddy/dql/commit/5d5c941ef640b80e0b572b470973580b687bc713) Client has option to run DQL from a file
- [5590f15](https://github.com/jspreddy/dql/commit/5590f15bb9db2654fe00f51d5ddfcfb3c1e27b2c) Adding query help docs to the cli
- [48d1ae8](https://github.com/jspreddy/dql/commit/48d1ae859359015f6fba3f676578bd3ba9051ef9) Updating docs for the scan 'IN' filter
- [1c5ee1d](https://github.com/jspreddy/dql/commit/1c5ee1d5c4b8ace8fd2cf518b40f32c7b310f097) Changing setup.py version detection
- [683f8c0](https://github.com/jspreddy/dql/commit/683f8c0c19864aca968dfe11d6f7dce9264c5ec1) Adding more of the WHERE and FILTER options
- [b14ffcd](https://github.com/jspreddy/dql/commit/b14ffcda83790b9fb7262ac3dd61295d931079bd) Better query string formatting
- [296b688](https://github.com/jspreddy/dql/commit/296b688bcf242449a0ab589216f501c634a9acf9) The client now supports queries over multiple lines
- [f413711](https://github.com/jspreddy/dql/commit/f413711341c640b859dc3de705d4e9c92ebd9555) Fixing coveralls link in README
- [02aea3b](https://github.com/jspreddy/dql/commit/02aea3b51b185eb381bf792b944045c6a5ff8ae6) Updating links in the README
- [d700006](https://github.com/jspreddy/dql/commit/d700006d46874c53d6df7f157ea1a97c3985d38d) Adding sphinx apidocs
- [8adeef3](https://github.com/jspreddy/dql/commit/8adeef34eca54abe8c814045ed34a048263debcc) Adding a bunch of documentation
- [b6dc5af](https://github.com/jspreddy/dql/commit/b6dc5af37c5a89990e8aeb24f098be6375d126c3) Engine uses correct index name by default
- [2c322ed](https://github.com/jspreddy/dql/commit/2c322ede1531d61d6cb4a5c707ef337324b768a9) Select and count can force consistent reads
- [740474a](https://github.com/jspreddy/dql/commit/740474a73ef47a1b92e33bcd6ee98ae7b359aaf2) Select can reverse order of results
- [e07a1ac](https://github.com/jspreddy/dql/commit/e07a1ac49a91fc1a5467617ad8d6c028663ae771) Engine can run multiple statements separated by ';'
- [c8a2cdc](https://github.com/jspreddy/dql/commit/c8a2cdce3191990737d1856473e2f3a9cd133032) Found a bug in schema dumping
- [cddde0a](https://github.com/jspreddy/dql/commit/cddde0abc26eeb1ab21ab45fb7f2438f56bf7f2e) Adding a command to dump table schemas
- [3353afd](https://github.com/jspreddy/dql/commit/3353afd66fd977c7dd138f525bafc398a3b025aa) Adding an ALTER statement to change throughput
- [245ff30](https://github.com/jspreddy/dql/commit/245ff30723f1405416ddbc5d97d5662e66a8bdd1) Create statement can specify throughput
- [941454a](https://github.com/jspreddy/dql/commit/941454acdc102a4729daaf42bb126d39655f29ab) Removing dict comprehension for python 2.6
- [53816b9](https://github.com/jspreddy/dql/commit/53816b9e5e69b48409c7b505d1e8ebc8b964ed97) Removing dict comprehension for python 2.6
- [b350bc3](https://github.com/jspreddy/dql/commit/b350bc39beacc642dc9aff18cac22752c09b6b26) Adding an update statement
- [0c3e201](https://github.com/jspreddy/dql/commit/0c3e201c7ce873167faca3926bd7839110dd72e5) Adding syntax for doing a batch get with SELECT
- [5ca7030](https://github.com/jspreddy/dql/commit/5ca7030ede62ec14004fe708d7baf271736b1a8e) Better error reporting for repl
- [5e6bc3b](https://github.com/jspreddy/dql/commit/5e6bc3b43d092a3f0b6b582c30b840d8679ae020) Adding a SCAN statement
- [858f2a4](https://github.com/jspreddy/dql/commit/858f2a4d6e02d74d3a9a27f44cafbbcbe8b51c9a) Adding a new query for counting items
- [7d01b6f](https://github.com/jspreddy/dql/commit/7d01b6f2e5c22adce670d096d6b044b8379c28e8) Select statement can fetch a subset of attributes
- [21cf476](https://github.com/jspreddy/dql/commit/21cf476166e673e39f4ce61a0bb2b5fb8d258b1b) Adding a 'limit' clause to the select query
- [8a2f2fc](https://github.com/jspreddy/dql/commit/8a2f2fcceb24a6067ea16b422d16a6cd9f420169) Updating the TODO
- [31f5fdb](https://github.com/jspreddy/dql/commit/31f5fdb168b8026595a4a2ea1eb3b9ea0f394858) Fixing a bug in the query compiler
- [f20bbec](https://github.com/jspreddy/dql/commit/f20bbec83dc40c236f47e86693a39fdbac1f2252) Use unittest2 on python 2.6
- [d41cc1a](https://github.com/jspreddy/dql/commit/d41cc1a0bed04c096eb2895e7d56cb59fe0cb2c3) Require argparse on python 2.6
- [8d27b1c](https://github.com/jspreddy/dql/commit/8d27b1c40e13f16b12ef5646ab57b43329cc80db) Putting tox requirement into travis.yml file
- [454de8b](https://github.com/jspreddy/dql/commit/454de8b9d52c2943329d28dfaee13ee0b944a28e) Adding tox as a dev requirement
- [a7704ac](https://github.com/jspreddy/dql/commit/a7704ac34b80cff16f06144175d45b556bffb2d5) Adding support for python 2.6
- [d106e9d](https://github.com/jspreddy/dql/commit/d106e9dd6f6b96b9a008cada2b2aeb645c21511e) Adding coveralls badge to README
- [2c2e5cf](https://github.com/jspreddy/dql/commit/2c2e5cf08923d402b879e528ca53ad7c30f69ef8) Adding travis badge to README
- [0624689](https://github.com/jspreddy/dql/commit/06246893b39ede009142b8ad41778120e8bec769) And so begins the tweaking of shit to make the travis build pass
- [3eaf6dc](https://github.com/jspreddy/dql/commit/3eaf6dc9dff45da3ebe203c01b06a0246edc5995) Allow passing in AWS credentials
- [f466617](https://github.com/jspreddy/dql/commit/f46661773a02b944de158298652c71e501c51d3f) First code commit
- [2c3bc57](https://github.com/jspreddy/dql/commit/2c3bc57146a4a1df588b2f51dec973364ed6d812) Initial commit
