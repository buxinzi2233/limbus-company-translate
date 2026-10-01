# 翻译研究资料索引

随仓库与预编译包分发的 **65 份历史研究报告**。这些资料用于寻找译法线索，不是经过认证的官方设定、零协会规则、当前任务指令或人工审批。

## 来源与可信度

来自本项目既有汉化研究工作区的历史研究归档，按原分类保留。部分角色例句、剧情推断和术语结论未经核实，可能过时或错误；必须回到当前韩文、说话人、上下文及可定位的公开来源复核。旧审计中的生产统计不能作为新工作区的进度。

公开导出仅增加历史警示、去除历史本机绝对路径及不可分发的本机证据链接；未以修改报告正文的方式替其中结论背书。逐文件原始哈希、导出哈希及修改说明见 [catalog.json](catalog.json)。原始本机工作库、私人审校证据和游戏资源没有随报告上传；被移除的证据不可视为已验证。

正文中的游戏文本、社区引用与外链内容保留其原权利归属；仓库 MIT 许可证不替代这些第三方内容的许可。公开参考快照许可另见 [语料清单](../corpus/manifest.json)。

## AI 如何读取

新工作区脚本自动为 MCP 配置 `--research-dir`；已有客户端重连后使用 `list_research` 分页查看目录、`search_research` 按字面片段搜索、`read_research` 按返回的文档 ID 读全文，详见 [接口说明](../docs/AI_AGENT_INTEGRATION.md)。三者均只读，结果携带历史警示。服务启动时验证全部文件哈希；缺文件或损坏时明确拒绝启动，不静默跳过。

## 文件目录

### 翻译理念、文风与格式规范（5 份）

- [都市零协会本地化研报：韩汉跨语际句法转译与语气映射法则 (Korean-to-Chinese Syntax & Speech Level Guide)](legacy/01_philosophy_and_standards/report_korean_chinese_syntax_patterns.md)
- [都市零协会汉化规范调研报告（一）：符号与排版格式规约](legacy/01_philosophy_and_standards/report_punctuation_and_formatting.md)
- [都市零协会汉化规范调研报告（三）：战斗技能类文本翻译与排版规约](legacy/01_philosophy_and_standards/report_skills_translation.md)
- [都市零协会本地化研报：多模态语域分化翻译风格深度指南 (LLC Multi-Register Style Guide)](legacy/01_philosophy_and_standards/report_zeroasso_style_guide.md)
- [零协会翻译规范与本项目的语义忠实原则](legacy/01_philosophy_and_standards/report_zeroasso_translation_philosophy.md)

### 角色口吻与观察日志（27 份）

- [堂吉诃德言行观察：身份阶段、称呼与证据](legacy/02_character_personas/observation/obs_don_quixote.md)
- [浮士德观察记录：来源与格式](legacy/02_character_personas/observation/obs_faust.md)
- [格里高尔言行观察：异化、战争记忆与具体语境](legacy/02_character_personas/observation/obs_gregor.md)
- [希斯克利夫言行观察：原文否定句、记忆与口吻](legacy/02_character_personas/observation/obs_heathcliff.md)
- [以实玛利言行观察：航海经历、幸存者与口吻](legacy/02_character_personas/observation/obs_ishmael.md)
- [默尔索观察记录：条目、收尾与证据边界](legacy/02_character_personas/observation/obs_meursault.md)
- [奥提斯观察记录：来源与格式](legacy/02_character_personas/observation/obs_outis.md)
- [罪人观察记录与昵称：来源及使用边界](legacy/02_character_personas/observation/obs_overview_and_nicknames.md)
- [罗佳言行观察：收税人事件、称呼与解释边界](legacy/02_character_personas/observation/obs_rodion.md)
- [良秀语言观察：缩写及演出证据边界](legacy/02_character_personas/observation/obs_ryoshu.md)
- [11号罪人辛克莱：语言观察与剧情核验](legacy/02_character_personas/observation/obs_sinclair.md)
- [李箱观察记录：来源与格式](legacy/02_character_personas/observation/obs_yi_sang.md)
- [角色语言风格深度调研手册：01 号罪人 · 李箱（Yi Sang）](legacy/02_character_personas/sinners/01_yi_sang.md)
- [角色语言风格深度调研手册：02 号罪人 · 浮士德（Faust）](legacy/02_character_personas/sinners/02_faust.md)
- [角色语言风格深度调研手册：03 号罪人 · 堂吉诃德（Don Quixote）](legacy/02_character_personas/sinners/03_don_quixote.md)
- [角色语言风格深度调研手册：04 号罪人 · 良秀（Ryōshū）](legacy/02_character_personas/sinners/04_ryoshu.md)
- [角色语言风格深度调研手册：05 号罪人 · 默尔索（Meursault）](legacy/02_character_personas/sinners/05_meursault.md)
- [角色语言风格深度调研手册：06 号罪人 · 鸿璐（Hong Lu）](legacy/02_character_personas/sinners/06_hong_lu.md)
- [角色语言风格深度调研手册：07 号罪人 · 希斯克利夫（Heathcliff）](legacy/02_character_personas/sinners/07_heathcliff.md)
- [角色语言风格深度调研手册：08 号罪人 · 以实玛利（Ishmael）](legacy/02_character_personas/sinners/08_ishmael.md)
- [角色语言风格深度调研手册：09 号罪人 · 罗佳（Rodion）](legacy/02_character_personas/sinners/09_rodion.md)
- [角色语言风格深度调研手册：11 号罪人 · 辛克莱（Sinclair）](legacy/02_character_personas/sinners/10_sinclair.md)
- [角色语言风格深度调研手册：12 号罪人 · 奥提斯（Outis）](legacy/02_character_personas/sinners/11_outis.md)
- [角色语言风格深度调研手册：13 号罪人 · 格里高尔（Gregor）](legacy/02_character_personas/sinners/12_gregor.md)
- [角色语言风格深度调研手册：管理者 · 但丁（Dante）](legacy/02_character_personas/sinners/13_dante.md)
- [角色语言风格深度调研手册：14 号核心 · 维吉里乌斯（Vergilius）](legacy/02_character_personas/sinners/14_vergilius.md)
- [角色语言风格深度调研手册：同行角色 · 卡戎（本文排序15）（Charon）](legacy/02_character_personas/sinners/15_charon.md)

### 文学原型与章节研究（3 份）

- [默尔索（Meursault）文学原型与加缪哲学深度考据研报](legacy/03_season8_literary_research/01_camus_archetype.md)
- [05号罪人 · 默尔索（Meursault）官方设定、PV轨迹与E.G.O深度研报](legacy/03_season8_literary_research/02_official_pv_and_settings.md)
- [第8赛季“刺点”与第10章（Canto X）深度剧情考据及汉化校勘研报](legacy/03_season8_literary_research/03_canto10_lore_and_audit.md)

### 语料、术语与社区反馈（3 份）

- [社区反馈研究：证据状态与可用原则](legacy/04_corpus_and_terminology/report_community_feedback_and_philosophy_evolution.md)
- [角色语料核验与统计使用边界](legacy/04_corpus_and_terminology/report_in_game_corpus_analysis.md)
- [都市零协会汉化规范调研报告（二）：命名存档体系与术语库研报](legacy/04_corpus_and_terminology/report_name_archive.md)

### 章节梗概（11 份）

- [章节剧情与设定考据档案：章节I · 无归属者（Canto I: The Outcast）](legacy/05_canto_plot_compendium/canto_01_gregor.md)
- [章节剧情与设定考据档案：章节II · 无法去爱（Canto II: The Unloving）](legacy/05_canto_plot_compendium/canto_02_rodion.md)
- [章节剧情与设定考据档案：章节III · 未曾面对（Canto III: The Unconfronting）](legacy/05_canto_plot_compendium/canto_03_sinclair.md)
- [章节剧情与设定考据档案：章节IV · 无改变者（Canto IV: The Unchanging）](legacy/05_canto_plot_compendium/canto_04_yi_sang.md)
- [章节剧情与设定考据档案：章节V · 定义为恶（Canto V: The Evil Defining）](legacy/05_canto_plot_compendium/canto_05_ishmael.md)
- [章节剧情与设定考据档案：章节VI · 心意相悖（Canto VI: The Heartbreaking）](legacy/05_canto_plot_compendium/canto_06_heathcliff.md)
- [章节剧情与设定考据档案：章节VII · 梦之终焉（Canto VII: The Dream's Ending）](legacy/05_canto_plot_compendium/canto_07_don_quixote.md)
- [章节剧情与设定考据档案：章节VIII · 不思观望（Canto VIII: The Sightless）](legacy/05_canto_plot_compendium/canto_08_hong_lu.md)
- [章节剧情与设定考据档案：章节IX · 无割舍者（Canto IX: The Unrelinquishing）](legacy/05_canto_plot_compendium/canto_09_ryoshu.md)
- [章节剧情与设定考据档案：章节X · 被凝视者（Canto X: The Gaze Bearing）](legacy/05_canto_plot_compendium/canto_10_meursault.md)
- [章节剧情与设定考据档案：中篇物语全景考据（Intervallos Compendium）](legacy/05_canto_plot_compendium/canto_intervallos.md)

### 世界观与理论（5 份）

- [都市世界观参考：卡门、扭曲、罪种与 E.G.O](legacy/06_worldbuilding_and_theories/report_carmen_distortion_ego_mechanisms.md)
- [都市世界观深度考据研报：五指黑道教条与后巷地下生态](legacy/06_worldbuilding_and_theories/report_fingers_and_syndicates.md)
- [都市世界观深度考据研报：收尾人等级、十二协会与特色收尾人谱系](legacy/06_worldbuilding_and_theories/report_fixers_associations_and_colors.md)
- [都市世界观深度考据研报：译名依据与本地约定](legacy/06_worldbuilding_and_theories/report_localization_disputes_and_consensus.md)
- [都市世界观参考：翼、已知技术与奇点的证据边界](legacy/06_worldbuilding_and_theories/report_wings_and_singularities.md)

### 哲学脉络（9 份）

- [大陆意志哲学·存在主义与荒谬谱系总论与思想演进图谱](legacy/07_philosophical_lineage/00_overview_and_genealogical_map.md)
- [存在之先声：克尔凯郭尔与陀思妥耶夫斯基考据与翻译手册](legacy/07_philosophical_lineage/01_the_precursors_kierkegaard_and_dostoevsky.md)
- [阿图尔·叔本华：生存意志、悲观论与解脱之道考据手册](legacy/07_philosophical_lineage/02_schopenhauer_will_and_pessimism.md)
- [弗里德里希·尼采：权力意志、重估一切与虚无主义考据手册](legacy/07_philosophical_lineage/03_nietzsche_will_to_power_and_nihilism.md)
- [现象学与本体论基石：胡塞尔·海德格尔·雅斯贝尔斯考据手册](legacy/07_philosophical_lineage/04_phenomenology_and_ontology_husserl_heidegger_jaspers.md)
- [让-保罗·萨特与西蒙娜·德·波伏娃：存在先于本质、自由与他者考据手册](legacy/07_philosophical_lineage/05_sartre_and_beauvoir_existentialism_and_freedom.md)
- [阿尔贝·加缪：荒诞哲学、西西弗斯、反抗者与南方思想考据手册](legacy/07_philosophical_lineage/06_camus_the_absurd_and_rebellion.md)
- [埃米尔·齐奥朗：极致虚无、绝望之巅与怀疑主义考据手册](legacy/07_philosophical_lineage/07_cioran_extreme_nihilism_and_despair.md)
- [欧陆哲学多语术语研究对照表（非强制词典）](legacy/07_philosophical_lineage/08_multilingual_philosophical_lexicon.md)

### 历史调查与范围审计（2 份）

- [翻译参考库：当前状态与证据](legacy-audits/knowledge-base-audit.md)
- [Part 2 汉化更新范围确认](legacy-audits/part2-update-scope.md)
