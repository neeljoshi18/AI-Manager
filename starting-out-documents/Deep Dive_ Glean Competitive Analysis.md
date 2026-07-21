# **Technical Audit and Competitive Strategy Report: Demystifying Glean's Architecture for Target-Defensible Counter-Positioning**

## **Executive Summary & Competitive Thesis: The "Strip-to-Win" Leverage**

The landscape of enterprise knowledge management and cognitive retrieval is dominated by broad horizontal platforms, of which Glean Technologies represents the premier incumbent.1 Reaching a $7.2 billion valuation with annual recurring revenue (ARR) surpassing $200 million as of mid-2025 2, Glean's trajectory demonstrates the massive demand for centralized organizational search.4 However, its core strength—an exhaustive, horizontal indexing footprint that connects to over 275 SaaS applications and maps entire company directories—is also its primary structural vulnerability.3  
Glean’s all-encompassing strategy forces an expensive, high-touch sales and deployment architecture.6 The typical entry point requires minimum contracts of 100 seats, bringing baseline annual commitments to $50,000–$60,000, with fully-loaded Total Cost of Ownership (TCO) for mid-to-large enterprises routinely scaling to $350,000–$480,000 annually.8 This extensive pricing structure covers a massive engineering and operational stack: single-tenant VPC infrastructure hosting, continuous permissions mapping, high-compute vector embeddings generation, optical character recognition (OCR) processing, and administrative support.8  
The critical opening for a disruptive market entrant lies in a targeted "strip-to-win" strategy.6 In practice, many departments within an enterprise do not actively use or require the exhaustive retrieval footprint offered by Glean.6 By isolating a single, high-friction operational vertical—specifically, hyper-growth engineering teams scaling past the 50-employee threshold—a specialized platform can deliver superior workflow outcomes at a fraction of the cost.14  
The Autonomous AI Manager Platform counter-positions against Glean not by attempting to replicate its massive, cross-department indexing web, but by functioning as an anti-engagement context layer.14 Where Glean seeks to maximize chat time and search actions across a sprawling web portal 6, the AI Manager's success is measured strictly by focus time reclaimed, aiming to pull employees off communication platforms entirely and replace high-frequency status-sync meetings with passive, background organizational synchronization.14 By storing only metadata and collaborative patterns within a localized Organizational Context Graph, rather than indexing full documents or hosting proprietary source code, a startup can eliminate up to 90% of the database, compute, and licensing costs that inflate Glean’s pricing.6

## **Exhaustive Functional Audit of Glean's Enterprise Feature Set**

Glean’s platform architecture is designed as a unified three-layer system: a data ingestion and connector layer, an understanding and semantic graph layer, and an application and agent execution layer.18 The following audit details every verified feature, connector behavior, and operational mechanism in Glean's 2026 suite.

### **Cognitive Retrieval & Search Architecture**

#### **Hybrid Retrieval Engine**

Glean’s core search relies on the concurrent execution of sparse lexical matching and dense vector semantic search.19 Lexical scoring is powered by the Best Match 25 (BM25) algorithm, which evaluates term frequency (![][image1]), inverse document frequency (![][image2]), and document length normalization.22 The mathematical structure is expressed as:  
![][image3]  
where ![][image4] regulates term-frequency saturation, ![][image5] represents the document length penalty (defaulting to 0.75), and ![][image6] represents the document length ratio relative to the corpus average.23 The logarithmic inverse document frequency is defined as:  
![][image7]  
where ![][image8] is the number of documents containing the term across a total corpus of ![][image9] documents.23 Semantic search maps queries and documents into a high-dimensional vector space using transformer models, calculating relevance via high-dimensional cosine distance.20 The outputs of both retrievers are merged using weighted scoring algorithms to prioritize documents that exhibit both exact token relevance and semantic alignment.21

#### **Decoupled Search Planning (The Waldo Engine)**

To control latency and token consumption, Glean separates search planning from final answer synthesis.25 The platform routes all initial natural-language queries through *Waldo*, a low-parameter search-planning model built on NVIDIA Nemotron 3 Nano.26 Waldo autonomously parses the user query, determines which application connectors to query, executes search loops, evaluates if the retrieved context is sufficient, and compiles the relevant text segments.26 This planning layer reduces query latency by 50% and token consumption by 23% to 25%, ensuring that high-parameter frontier models (e.g., GPT-4 or Claude 3.5 Sonnet) are only invoked for final text generation and citation formatting.25

### **Context Graphs & Voice Emulation**

#### **Enterprise Graph**

Glean processes crawled data and metadata to map the entire organizational taxonomy.5 This dynamic model identifies high-value entities (employees, teams, document categories, projects, code repositories, tickets) and establishes relational edges ("owns," "reports to," "collaborated on," "referenced by").5 This allows the search engine to perform multi-hop reasoning, distinguishing between identical terms based on local project contexts.5

#### **Personal Graph**

Maintains a granular user-activity database, tracking edits, pings, Slack replies, turnaround times, and calendar contexts.30 It leverages this data to construct personal relevance rankings and generate personalized activity cards on the user's home screen.3

#### **Voice Emulation Profiles**

The Personal Graph categorizes each user's unique communication patterns into up to 5 distinct writing profiles depending on the target audience (e.g., informal team chats, technical commits, formal stakeholder emails).30 When drafting content, Glean automatically fetches the correct profile to match the user's personal tone and style.30

### **Agentic Execution & Process Automation**

#### **Unified Agent Builder**

A low-code creation environment that merges conversational natural-language prompt engineering with graph-based step-by-step workflow mapping.32 Creators can alternate between chatting with the builder to add nodes and manually configuring specific API variables.32

#### **Fast vs. Thinking Planning Modes**

Within plan-and-execute nodes, agent creators can optimize for speed or depth.32 "Fast Mode" executes rapid, direct database updates or lookups.32 "Thinking Mode" activates advanced reasoning paths via Glean's Agentic Engine 2\.32 In Thinking Mode, the agent proposes an execution plan, queries connected tools, evaluates results, self-corrects upon error, and modifies its plan in real-time.32 In benchmark testing, Thinking Mode achieved a 94% end-to-end completion rate on complex analytical workflows.32

#### **Reusable Skills Framework**

Skills represent packaged, reusable blocks of execution logic containing step instructions, tool parameters, and structural constraints.31 Rather than keeping thousands of tokens of tool descriptions constantly in the model's active context window, Glean uses progressive schema hydration.34 It queries its internal index to locate matching skills, shows the model a short list of lightweight descriptions, and only injects the full schema at the exact moment the model commits to executing that specific step.34

#### **Ephemeral Sandbox & Programmatic Tool Calling (PTC)**

For complex workflows, Glean executes Python scripts in an isolated, single-session container.35 PTC replaces slow, expensive model round-trips by prompting the LLM to write a single orchestration script.36 This script executes loops, branches, and API pagination directly within the secure sandbox, filtering thousands of records before returning a concise, consolidated output to the model's context window.36

#### **Excerpt of Core Agent Templates**

| Agent Template Name | Target Department | Operational Mechanism |
| :---- | :---- | :---- |
| **Spec to Implementation PR** 37 | Engineering | Parses a design specification document, maps the logic to existing repositories, and drafts a review-ready GitHub PR.37 |
| **Engineering Standup** 37 | Engineering | Scans the developer's Git trees, Jira history, and Slack logs to compile an impact-focused daily standup update.14 |
| **Ghostwriter** 37 | All Teams | Drafts cross-platform communications leveraging the Personal Graph's voice profiles and past writing context.30 |
| **Support Doc from Ticket** 37 | IT & Support | Automatically transforms resolved customer support tickets into structured help center articles.37 |
| **Account Snapshot** 37 | Sales & Success | Aggregates CRM updates, recent emails, and support histories to produce a comprehensive client health profile.37 |

### **Model Context Protocol (MCP) Integration**

#### **Glean MCP Server**

Acts as a remote, managed implementation of the Model Context Protocol.40 Deployed inside the client's isolated VPC, it exposes search, chat, and custom agents as standard JSON-RPC tools over HTTPS/SSE endpoints.40 This allows external IDEs (Cursor, VS Code) or CLI terminals (Claude Code, Gemini CLI) to use Glean's permission-aware context directly within the developer's local coding interface.40

#### **Glean MCP Gateway**

A reverse-proxy layer that lets administrators connect third-party MCP servers (e.g., Notion, Asana).41 It applies centralized access control, ensuring users only see tools they are personally permitted to use.41 The gateway enforces Glean Protect security guardrails on every outgoing call and tracks tool usage on an administrative dashboard.41 Glean recommends limiting active tools to 40 per MCP server to avoid context distraction.41

### **Engineering-Specific Tooling**

#### **Glean Code Search**

Maintains semantic and lexical indices over more than 100 million code files.28 Operating with a 50 ms P95 latency, it parses repository hierarchies and tracks developer co-edit histories to prioritize active, reference-heavy files over obsolete code blocks.28

#### **Code Writer**

Directly connects to GitHub to apply automated changes.38 Based on a ticket summary or stack trace, Code Writer plans changes, implements edits that follow local style guides, and opens a draft pull request complete with markdown summaries and documentation links.38

### **Security & Governance (Glean Protect)**

#### **Data Isolation**

Offers fully isolated, single-tenant hosting models either managed by Glean on GCP or hosted directly within the customer's AWS/GCP cloud environment.11

#### **Enforced Permissions**

Enforces source-application permissions in real-time, preventing users from seeing any content they cannot access in the source system.11

#### **Sensitive Content Detection**

Provides customizable policies to detect and triage payment credentials, health data, or developer tokens across all indexed sources.11

#### **Runtime Agent Guardrails**

Uses security models to pre-scan write tools and block misaligned agent behavior, jailbreak attempts, and prompt injections.11

## **Strategic "Build vs. Strip" Analysis for "The AI Manager" Platform**

To challenge Glean, the AI Manager must reduce TCO by limiting its functional scope.8 The table below evaluates which of Glean’s major features are necessary to build, which are represented in the AI Manager's pitch deck, and which must be stripped out to achieve a low-cost, high-margin offering.8

| Feature / Capability | In AI Manager PDF? | Implementation Decision | Technical Overhead & Cost Reduction Justification |
| :---- | :---- | :---- | :---- |
| **Hybrid Retrieval (BM25 \+ Dense Vectors)** 19 | No 14 | **Strip** | **High Overhead.** Hosting local vector databases (FAISS, Pinecone) and computing embeddings for millions of enterprise documents requires heavy GPU infrastructure and ongoing administrative tuning.8 Stripping this avoids massive cloud storage and compute costs.8 |
| **Enterprise Graph (Entity Mapping)** 5 | Yes 14 | **Build (Metadata Only)** | **Moderate Overhead.** Rather than mapping all cross-department SaaS objects 5, build a specialized *Organizational Context Graph* restricted purely to developer exhaust (Git trees, Jira statuses, Slack logs).14 Storing metadata and relational connections—rather than full file contents—reduces database storage requirements by over 90%.8 |
| **Personal Graph & Voice Emulation** 30 | Yes 14 | **Build (Collaborative Patterns Only)** | **Low Overhead.** Replicate the mapping of team collaboration paths, decision trails, and response patterns.14 Strip the complex 5-profile voice emulation engine 30, focusing instead on developer status ledger compilation.14 |
| **Thinking Mode Reasoning** 32 | Yes 14 | **Build (Agent-to-Agent Focus)** | **Moderate Overhead.** While Glean uses general reasoning for multi-step data synthesis 32, implement specialized, background *Digital Twins* that virtualize and negotiate developers' status ledgers without requiring expensive, continuous calls to high-parameter LLMs.14 |
| **Skills Framework (Hydration)** 34 | No 14 | **Strip** | **Low Overhead.** Highly valuable for horizontal platforms with thousands of tool schemas.34 Because the AI Manager utilizes a tightly constrained set of first-party developer tools 14, progressive hydration is unnecessary. |
| **Ephemeral Sandbox & PTC** 36 | No 14 | **Strip** | **High Overhead.** Operating containerized sandboxes with active Python interpreters adds significant operational complexity and security risks.36 By focusing on read-only API parsing of Git and Jira metadata 14, the AI Manager avoids the need for virtual execution runtimes.36 |
| **MCP Gateway & Server** 40 | Yes 14 | **Build (Ingestion Only)** | **Low Overhead.** Ingest data via lightweight, secure Model Context Protocol (MCP) server endpoints.14 Avoid building a complex reverse-proxy gateway for third-party tools 41, keeping the codebase lean and focused.14 |
| **Glean Code Search** 28 | No 14 | **Strip** | **High Overhead.** Maintaining semantic search over 100 million active code files requires immense compute power and low-latency indices.28 The AI Manager should read Git metadata, change metrics, and commit trees rather than indexing and searching entire codebases line-by-line.14 |
| **Code Writer (GitHub PRs)** 38 | No 14 | **Strip** | **High Overhead.** Generating code modifications and opening PRs carries high error-rates and compliance friction.38 Stripping this keeps the AI Manager focused on organizational transparency and meeting elimination.14 |
| **Glean Protect (Sensitive DLP)** 11 | Yes 14 | **Build (Governance Only)** | **Low Overhead.** Implement standard, rule-based API governance to ensure the platform never clones or permanently stores proprietary source code.14 Avoid building complex machine learning models for PII/PCI detection.11 |

### **Architectural Cost Efficiencies**

By stripping full-text indexing, vector search hosting, high-compute document ingestion pipelines, OCR processing, and multi-session sandboxes 8, the AI Manager can operate a highly efficient database architecture. Storing only collaborative metadata, Git tree configurations, and status ledgers requires minimal cloud compute.14 This allows the platform to run on serverless cloud databases (e.g., PostgreSQL with lightweight JSON metadata tracking), eliminating the $10,000/month infrastructure hosting costs that inflate Glean's TCO.8

## **Technical Deep Dive: Permissions, Crawling, and API Integration Mechanics**

### **Data Ingestion and Crawling Schedules**

Glean’s data freshness and sync capabilities are achieved through a three-tier crawling architecture.44 Understanding this pipeline reveals the baseline API dependency model.

   
       |  
       \+---\> Webhook Events (Real-Time Adds/Updates/Permissions) \---\> Ingestion Queue (1-5 min Sync) \[47, 48\]  
       |  
       \+---\> Incremental API Crawl (Every 10 Minutes) \---------------\> Sync Audit & Catch-Up \[47\]  
       |  
       \+---\> Full Metadata Crawl (Every 28 Days) \-------------------\> Deep Index Reconstruction \[47\]

This crawling pipeline is heavily limited by rate limiting and indexing constraints:

* **API Quota Exhaustion:** Integrations with platforms like Jira, Box, and SharePoint share API rate-limit thresholds across all connected apps.47 If a customer has a large number of files owned by a single service account, Glean's crawlers can hit these limits, stalling search synchronization and disrupting downstream corporate applications.47  
* **Document Constraints:** The Indexing API rejects any document containing a body larger than 100KB, returning a 400 Document too large error.48 This requires upstream pre-processing and document truncation.48  
* **Namespace Restrictions:** Native and custom data sources are restricted to separate namespaces.51 Pushed content cannot use a native datasource name.51 This namespace separation means that any migration from a native connector to a custom connector requires manual re-configuration of user-facing search filters, pinned results, and custom agents.51

### **Identity Providers & Permissions Mapping**

Glean reconstructs security access lists (ACLs) by mapping enterprise directory identities from Okta or Entra ID to local SaaS application user profiles.19 This allows the platform to enforce real-time permission checks at query time.19  
This mapping mechanism introduces significant friction and permission bottlenecks, particularly when integrating with Microsoft SharePoint and Entra ID 55:

* **The FullControl Scope Bottleneck:** To detect permission-only changes in real-time, Glean requires the highly elevated Sites.FullControl.All permission scope across the SharePoint REST and Graph APIs.55 Without this scope, Microsoft's APIs do not return permission change events.57 This forces Glean to fall back to a 24-hour incremental crawl to update permissions, exposing a potential security gap.57 However, requesting FullControl often triggers significant security and compliance pushback from enterprise IT teams.57  
* **The Selected Sites Friction:** If security teams force the use of Sites.Selected to restrict Glean's crawling scope 55, the platform cannot auto-discover new directories.55 Every single new SharePoint site and sub-site must be manually added to both the Azure App Registration and the Glean Admin Console.55 This manual process introduces ongoing administrative friction that can hinder platform expansion.55

### **The AI Manager Platform Approach**

To minimize this integration and compliance friction, the AI Manager platform should adopt a streamlined, low-privilege integration model.14

> 1. **Low-Privilege API Scopes:** Restrict access exclusively to developer repositories (GitHub, GitLab) and engineering-focused communication channels (Slack, Microsoft Teams).14 Avoid wide-ranging directory scopes like User.Read.All or global storage permissions like SharePoint FullControl.56  
> 2. **Metadata-Only Ingestion:** The AI Manager platform should ingest only metadata, collaboration timelines, and commit trees, without reading or storing proprietary source code or document contents.14 This narrow scope reduces compliance reviews from weeks to days.3  
> 3. **Passive Shadow Mode:** By operating silently in background shadow mode during the first 10 days, the platform maps the team's true collaboration network using existing Slack and Git telemetry without requiring manual configuration or administrative setup.14

## **Deep Testimonial Analysis: Verified Strengths, Weaknesses, and the Relational Reasoning Wall**

### **Verified User Appreciations (The "Good")**

* **High Retrieval Precision:** Users consistently praise Glean's ability to search across highly fragmented tool stacks (Slack, Google Drive, Jira, GitHub) and return accurate, contextually relevant documents with sub-second latency.19  
* **Trusted Conversational Answers:** Generative responses are highly trusted because they are grounded in direct, permission-aware internal citations.2 Users can immediately click the provided links to verify sources in original applications.3  
* **Proactive Contextual Discovery:** The browser extension and homepage are widely adopted because they proactively surface relevant, trending documents based on the user's direct team and active projects.3

### **Core Criticisms & Operational Weaknesses (The "Bad")**

* **High TCO and Rigid Licensing:** The lack of transparent pricing and high minimum seat requirements (100 seats, \~$50,000–$60,000/year minimum) puts Glean out of reach for smaller teams.3 The per-seat pricing model penalizes companies for low adoption, as they must pay for all licensed users regardless of active usage.6  
* **Significant Setup and IT Maintenance Overhead:** Deploying the platform requires weeks of configuration, permission mapping, and administrative troubleshooting.3 Furthermore, if a company's internal data is poorly organized or suffers from oversharing, Glean will surface these governance failures.3  
* **The Execution Gap:** Despite the introduction of agentic workflows and write tools, users report that Glean remains a "search-first" platform.3 It excels at finding information, but frequently stops short of executing end-to-end tasks or automating complex workflows in downstream systems.3

### **The Relational Reasoning Wall: A Developer Case Study**

A critical technical limitation of standard enterprise search engines like Glean emerges when developers attempt to use them as a context layer for autonomous AI agents.61  
In a documented evaluation from the r/AgentsOfAI community, an engineering team spent three months trying to build active, ticket-resolving AI agents using Glean's enterprise search framework.61 They ran into a major technical barrier: **search engines are optimized to return document links to a human brain, but autonomous agents require structured relational paths and explicit lineage to reason over a problem**.61  
When an agent executes a complex, relational query—such as "track a client's project history and changing requirements across a year of Slack channels, Jira tickets, and SharePoint drafts"—standard semantic search performs flat vector segment extraction.61 It retrieves disjointed, out-of-order text chunks and dumps them into the agent's context window.61  
Because standard search indices lack a native concept of **temporal states** (how requirements evolved over time), **explicit lineage** (which file version or ticket superseded another), and **entity-to-entity relationships** (how a Slack decision relates to a Git commit), the agent receives a highly distracted and out-of-sequence context window.53 This structural limitation leads to:

* **Severe Agentic Hallucinations:** Deprived of explicit chronological lineage, the agent struggles to synthesize the current state of a project, causing it to make false assertions about requirements and milestones.61  
* **Context Poisoning & Confusion:** Outdated specifications are weighted equally with active code patterns, leading the agent to propose buggy or misaligned code modifications.28  
* **High Token Waste:** To resolve the missing lineage, agents are forced to run multiple, repetitive search queries, causing token usage to escalate and introducing significant latency (often 12+ seconds) before synthesis even begins.26

### **The AI Manager Platform Solution**

The AI Manager platform's *Organizational Context Graph* directly addresses this relational reasoning wall.14 Rather than searching for flat text chunks, the AI Manager’s agents query a dedicated graph database that explicitly models **time**, **lineage**, and **dependencies**.14 By mapping developer exhaust directly to virtual Digital Twins, the platform maintains a structured, chronological path of project health and decisions.14 This ensures that agents receive complete context, minimizing hallucinations and token waste without the overhead of full-text document indexing.8

## **Structural Cost Modeling and Market Counter-Attack Blueprint**

To compete with Glean, the AI Manager platform must offer a transparent, lower-cost pricing model that targets underserved, fast-growing engineering teams.8

### **Cost Structure Comparison**

| Cost Metric / Parameter | Glean Platform Deployment Model | The AI Manager Platform Model | Competitive Advantage & Value Proposition |
| :---- | :---- | :---- | :---- |
| **Base Seat Price** | \~$50–$75 per user, per month.8 | **$15–$20 per user, per month**.10 | **60% to 70% direct licensing savings**.10 |
| **Contract Minimums** | Strict 100-seat minimums (\~$60,000 ACV floor).8 | **Flexible, no seat minimums**.10 | Opens the market to teams under the 100-user threshold, capturing startups and agile teams.6 |
| **Mandatory Support Fees** | Officially confirmed at 12% of licensing fees.2 | **Zero mandatory fees**.10 | Removes hidden cost inflation.6 |
| **Infrastructure Hosting** | Single-tenant VPC hosting (\~$120,000/year compute/storage).8 | **Shared or lightweight serverless metadata hosting**.14 | Storing only collaborative metadata—rather than full file content—reduces database hosting costs by over 90%.8 |
| **Paid Proof of Concept (POC)** | Paid-only pilot programs (costing up to $70,000).9 | **Free, self-serve 14-day trial**.10 | Removes friction from the sales cycle, enabling fast adoption without long sales negotiations.9 |
| **Fully-Loaded TCO (250 Users)** | **$350,000–$480,000 / year**.8 | **$45,000–$60,000 / year**.10 | Delivers massive cost reductions by stripping out generalized enterprise indexing and hosting overhead.8 |

### **Market Counter-Attack Blueprint**

Phase 1: Frictionless Integration  
 \---\> \---\> \---\>  
                                                                                                        |  
                                                                                                        v  
Phase 2: Meeting Elimination                                                                            |  
 \----\> \---\> \--+  
                                                                                  |  
                                                                                  v  
Phase 3: Scale & Expand                                                                           |  
 \----\> \---\> \[ Expand Across Engineering Orgs \]

#### **Phase 1: Infiltrate via Frictionless Integration**

Target engineering organizations approaching or scaling past the 50-employee threshold.14 Offer a free, self-serve 14-day trial with zero upfront setup fees or sales negotiations.9 Upon integration, the platform runs in a 10-day passive "shadow mode," silently ingesting developer metadata via secure API and MCP endpoints to map the team's collaboration patterns without administrative overhead.14

#### **Phase 2: Deliver Immediate High-ROI Meeting Elimination**

Deploy the platform's headless UX directly within the developers' native environments (IDE extensions, Slack/Teams DMs).14 Compile daily status ledgers using the confidence-tier framework 14:

* **High Confidence:** Status updates and dependency maps publish autonomously.14  
* **Medium Confidence:** Updates deploy on an opt-out schedule where silence implies consent, prompting developer action only for errors.14 This approach delivers immediate, measurable value by eliminating high-friction synchronous standups and status meetings.14

#### **Phase 3: Displace Incumbent Platforms**

Use "focus time reclaimed" as the primary sales metric.14 Show engineering leadership clear analytics proving how the platform has reduced alignment meetings and accelerated deployment speeds.14 By delivering superior, engineering-specific workflow automation at a fraction of Glean's TCO, the AI Manager can position itself as a lean, high-ROI alternative.3 This allows the platform to capture the fast-growing technology sector, scaling from a specialized engineering tool into a broader, cost-effective context layer.8

#### **Works cited**

> 1. AI Platform for Work | Glean Work AI for Enterprise, accessed July 21, 2026, [https://www.glean.com/platform](https://www.glean.com/platform)  
> 2. Glean \- Metronome, accessed July 21, 2026, [https://metronome.com/pricing-index/glean](https://metronome.com/pricing-index/glean)  
> 3. Glean Review 2026: Features, Pricing, and My Honest Experience \- Fritz ai, accessed July 21, 2026, [https://fritz.ai/glean-review/](https://fritz.ai/glean-review/)  
> 4. Leading enterprise AI solution \- Glean, accessed July 21, 2026, [https://www.glean.com/enterprise-ai](https://www.glean.com/enterprise-ai)  
> 5. Enterprise Graph: Powering AI with Deep Organizational Knowledge \- Glean, accessed July 21, 2026, [https://www.glean.com/enterprise-context/enterprise-graph](https://www.glean.com/enterprise-context/enterprise-graph)  
> 6. Glean Pricing: Costs, TCO & Alternative Breakdown for 2026 \- Coworker AI, accessed July 21, 2026, [https://coworker.ai/blog/glean-pricing](https://coworker.ai/blog/glean-pricing)  
> 7. App Integrations for Glean – Connect 275+ Apps Instantly, accessed July 21, 2026, [https://www.glean.com/platform/connectors](https://www.glean.com/platform/connectors)  
> 8. Glean Pricing Explained — And Why Buyers Want More Transparency \- GoSearch, accessed July 21, 2026, [https://www.gosearch.ai/blog/glean-pricing-explained/](https://www.gosearch.ai/blog/glean-pricing-explained/)  
> 9. Glean AI pricing: A 2025 guide to its real costs & alternatives \- eesel AI, accessed July 21, 2026, [https://www.eesel.ai/blog/glean-ai-pricing](https://www.eesel.ai/blog/glean-ai-pricing)  
> 10. Glean Pricing Calculator: Estimate Glean's Real Cost (2026) | Onyx AI, accessed July 21, 2026, [https://onyx.app/glean-cost-calculator](https://onyx.app/glean-cost-calculator)  
> 11. AI Security: Protecting Enterprise Data with Glean, accessed July 21, 2026, [https://www.glean.com/platform/security](https://www.glean.com/platform/security)  
> 12. Glean deployment models, accessed July 21, 2026, [https://docs.glean.com/get-started/prepare/about-deployment](https://docs.glean.com/get-started/prepare/about-deployment)  
> 13. Top AI solutions for automating document data extraction \- Glean, accessed July 21, 2026, [https://www.glean.com/perspectives/top-ai-solutions-for-automating-document-data-extraction](https://www.glean.com/perspectives/top-ai-solutions-for-automating-document-data-extraction)  
> 14. ai\_manager\_pitch\_deck.pdf  
> 15. A complete overview of Glean for enterprise AI \- eesel AI, accessed July 21, 2026, [https://www.eesel.ai/blog/glean](https://www.eesel.ai/blog/glean)  
> 16. Slack AI, anything promising yet? \- Reddit, accessed July 21, 2026, [https://www.reddit.com/r/Slack/comments/1fvb92v/slack\_ai\_anything\_promising\_yet/](https://www.reddit.com/r/Slack/comments/1fvb92v/slack_ai_anything_promising_yet/)  
> 17. Comparing costs scaling AI search solutions in 2026 \- Glean, accessed July 21, 2026, [https://www.glean.com/perspectives/comparing-costs-scaling-ai-search-solutions-in-2026](https://www.glean.com/perspectives/comparing-costs-scaling-ai-search-solutions-in-2026)  
> 18. Enterprise AI software: How to choose the right platform \- Glean, accessed July 21, 2026, [https://www.glean.com/enterprise-ai-software](https://www.glean.com/enterprise-ai-software)  
> 19. Workplace Search AI – Instantly Find Answers Across All Apps \- Glean, accessed July 21, 2026, [https://www.glean.com/enterprise-search](https://www.glean.com/enterprise-search)  
> 20. Semantic search vs keyword search impact on enterprise productivity \- Glean, accessed July 21, 2026, [https://www.glean.com/perspectives/semantic-search-vs-keyword-search-impact-on-enterprise-productivity](https://www.glean.com/perspectives/semantic-search-vs-keyword-search-impact-on-enterprise-productivity)  
> 21. Hybrid Search (BM25 \+ Vector Embeddings): The Best of Both Worlds in Information Retrieval | by Mahima Agarwal | Medium, accessed July 21, 2026, [https://medium.com/@mahima\_agarwal/hybrid-search-bm25-vector-embeddings-the-best-of-both-worlds-in-information-retrieval-0d1075fc2828](https://medium.com/@mahima_agarwal/hybrid-search-bm25-vector-embeddings-the-best-of-both-worlds-in-information-retrieval-0d1075fc2828)  
> 22. BM25 Relevance Scoring \- Azure AI Search \- Microsoft Learn, accessed July 21, 2026, [https://learn.microsoft.com/en-us/azure/search/index-similarity-and-scoring](https://learn.microsoft.com/en-us/azure/search/index-similarity-and-scoring)  
> 23. What is BM25 Full-Text Search? How Document Ranking Works | Spice AI, accessed July 21, 2026, [https://spice.ai/learn/bm25-full-text-search](https://spice.ai/learn/bm25-full-text-search)  
> 24. BM25 vs. Vector Search: Choosing the Right Retrieval Strategy for Production Systems, accessed July 21, 2026, [https://aloknecessary.github.io/blogs/bm25\_vs\_vector\_search/](https://aloknecessary.github.io/blogs/bm25_vs_vector_search/)  
> 25. Understanding Glean and Claude Enterprise hosting architectures, accessed July 21, 2026, [https://www.glean.com/perspectives/understanding-glean-and-claude-enterprise-hosting-architectures](https://www.glean.com/perspectives/understanding-glean-and-claude-enterprise-hosting-architectures)  
> 26. Exploring Gleans approach search planning vs deep reasoning in AI, accessed July 21, 2026, [https://www.glean.com/perspectives/exploring-gleans-approach-search-planning-vs-deep-reasoning-in-ai](https://www.glean.com/perspectives/exploring-gleans-approach-search-planning-vs-deep-reasoning-in-ai)  
> 27. The Glean knowledge graph, accessed July 21, 2026, [https://www.glean.com/resources/guides/glean-knowledge-graph](https://www.glean.com/resources/guides/glean-knowledge-graph)  
> 28. How Glean Code Search Works, accessed July 21, 2026, [https://docs.glean.com/security/how-code-search-works](https://docs.glean.com/security/how-code-search-works)  
> 29. Knowledge graph vs vector database: how to choose your AI foundation \- Glean, accessed July 21, 2026, [https://www.glean.com/blog/knowledge-graph-vs-vector-database](https://www.glean.com/blog/knowledge-graph-vs-vector-database)  
> 30. Personal Graph: AI-Powered Personalization by Glean, accessed July 21, 2026, [https://www.glean.com/enterprise-context/personal-graph](https://www.glean.com/enterprise-context/personal-graph)  
> 31. The enterprise AI coworker: Proactively manage tasks, execute ..., accessed July 21, 2026, [https://www.glean.com/blog/may-2026-launch](https://www.glean.com/blog/may-2026-launch)  
> 32. Glean Agents adapt to real-world complexity and are built to scale safely across your enterprise, accessed July 21, 2026, [https://www.glean.com/blog/glean-agents-nov-drop-2025](https://www.glean.com/blog/glean-agents-nov-drop-2025)  
> 33. How ai-powered agents learn: a beginner's guide \- Glean, accessed July 21, 2026, [https://www.glean.com/perspectives/how-ai-powered-agents-learn-a-beginners-guide](https://www.glean.com/perspectives/how-ai-powered-agents-learn-a-beginners-guide)  
> 34. The harness as the context manager \- Glean, accessed July 21, 2026, [https://www.glean.com/blog/harness-context-manager](https://www.glean.com/blog/harness-context-manager)  
> 35. Why Glean is the enterprise AI coworker for getting work done, accessed July 21, 2026, [https://www.glean.com/blog/ai-coworker-for-enterprises](https://www.glean.com/blog/ai-coworker-for-enterprises)  
> 36. Agent Sandbox & Programmatic Tool Calling \- Glean Docs, accessed July 21, 2026, [https://docs.glean.com/security/agent-sandbox-ptc](https://docs.glean.com/security/agent-sandbox-ptc)  
> 37. Glean AI Agent Library | Automate Workflows with AI Agents, accessed July 21, 2026, [https://www.glean.com/ai-agents/agent-library](https://www.glean.com/ai-agents/agent-library)  
> 38. Code Generation \- Glean Docs, accessed July 21, 2026, [https://docs.glean.com/user-guide/assistant/code-generation](https://docs.glean.com/user-guide/assistant/code-generation)  
> 39. What are digital assistants and how to choose the right one \- Glean, accessed July 21, 2026, [https://www.glean.com/blog/what-are-digital-assistants](https://www.glean.com/blog/what-are-digital-assistants)  
> 40. MCP Security, Data Flow, and Permissions \- Glean Docs, accessed July 21, 2026, [https://docs.glean.com/administration/platform/mcp/security](https://docs.glean.com/administration/platform/mcp/security)  
> 41. Glean MCP Gateway \- Glean Docs, accessed July 21, 2026, [https://docs.glean.com/administration/platform/mcp/mcp-gateway](https://docs.glean.com/administration/platform/mcp/mcp-gateway)  
> 42. Connect remote MCP servers to Glean, accessed July 21, 2026, [https://docs.glean.com/administration/tools/connect-remote-mcp-servers-to-glean](https://docs.glean.com/administration/tools/connect-remote-mcp-servers-to-glean)  
> 43. Spend less time searching and more time building with Glean code search and code writer, accessed July 21, 2026, [https://www.glean.com/blog/code-search-code-writer-jan-drop-2026](https://www.glean.com/blog/code-search-code-writer-jan-drop-2026)  
> 44. About connectors \- Glean Docs, accessed July 21, 2026, [https://docs.glean.com/connectors/about](https://docs.glean.com/connectors/about)  
> 45. Glean Too Expensive? 10 Alternatives Compared by Price & AI Power (2026), accessed July 21, 2026, [https://www.thunai.ai/blog/glean-alternatives-and-competitors](https://www.thunai.ai/blog/glean-alternatives-and-competitors)  
> 46. Glean Reviews 2026: Details, Pricing, & Features \- G2, accessed July 21, 2026, [https://www.g2.com/products/glean-technologies-glean/reviews](https://www.g2.com/products/glean-technologies-glean/reviews)  
> 47. Box \- Glean Docs, accessed July 21, 2026, [https://docs.glean.com/connectors/native/box/](https://docs.glean.com/connectors/native/box/)  
> 48. glean-common-errors | glean-pack \- ClaudePluginHub, accessed July 21, 2026, [https://www.claudepluginhub.com/skills/flight505-glean-pack-plugins-saas-packs-glean-pack/glean-common-errors](https://www.claudepluginhub.com/skills/flight505-glean-pack-plugins-saas-packs-glean-pack/glean-common-errors)  
> 49. Jira Cloud \- Glean Docs, accessed July 21, 2026, [https://docs.glean.com/connectors/native/jira/](https://docs.glean.com/connectors/native/jira/)  
> 50. Crawling & learning process \- Glean Docs, accessed July 21, 2026, [https://docs.glean.com/get-started/review/crawling-and-learning](https://docs.glean.com/get-started/review/crawling-and-learning)  
> 51. Migrate from a native connector to a custom connector \- Glean Docs, accessed July 21, 2026, [https://docs.glean.com/connectors/custom/migration-playbook](https://docs.glean.com/connectors/custom/migration-playbook)  
> 52. Select people data source \- Glean Docs, accessed July 21, 2026, [https://docs.glean.com/get-started/setup/sync-people-data](https://docs.glean.com/get-started/setup/sync-people-data)  
> 53. Context engineering AI: The foundation of reliable, high-performing models \- Glean, accessed July 21, 2026, [https://www.glean.com/blog/context-engineering-ai-the-foundation-of-reliable-high-performing-models](https://www.glean.com/blog/context-engineering-ai-the-foundation-of-reliable-high-performing-models)  
> 54. Integrate Glean with Okta, accessed July 21, 2026, [https://www.okta.com/integrations/glean/](https://www.okta.com/integrations/glean/)  
> 55. Permissions and security controls \- Glean Docs, accessed July 21, 2026, [https://docs.glean.com/connectors/native/sharepoint/security/controls](https://docs.glean.com/connectors/native/sharepoint/security/controls)  
> 56. Setup (Native) \- Glean Docs, accessed July 21, 2026, [https://docs.glean.com/connectors/native/sharepoint/setup](https://docs.glean.com/connectors/native/sharepoint/setup)  
> 57. Connector permissions \- Glean Docs, accessed July 21, 2026, [https://docs.glean.com/connectors/native/sharepoint/security/permissions](https://docs.glean.com/connectors/native/sharepoint/security/permissions)  
> 58. Slack \- Glean Docs, accessed July 21, 2026, [https://docs.glean.com/connectors/native/slack/setup/slack-connector](https://docs.glean.com/connectors/native/slack/setup/slack-connector)  
> 59. Glean Reviews & Ratings 2026 | Gartner Peer Insights, accessed July 21, 2026, [https://www.gartner.com/reviews/product/glean-596848899](https://www.gartner.com/reviews/product/glean-596848899)  
> 60. Overview and concepts \- Glean Docs, accessed July 21, 2026, [https://docs.glean.com/tools/tools-overview](https://docs.glean.com/tools/tools-overview)  
> 61. We tried Glean and it's not enough. What are people using instead? : r/AgentsOfAI \- Reddit, accessed July 21, 2026, [https://www.reddit.com/r/AgentsOfAI/comments/1ue58o4/we\_tried\_glean\_and\_its\_not\_enough\_what\_are\_people/](https://www.reddit.com/r/AgentsOfAI/comments/1ue58o4/we_tried_glean_and_its_not_enough_what_are_people/)  
> 62. Optimizing token consumption why routing matters more than cost \- Glean, accessed July 21, 2026, [https://www.glean.com/perspectives/optimizing-token-consumption-why-routing-matters-more-than-cost](https://www.glean.com/perspectives/optimizing-token-consumption-why-routing-matters-more-than-cost)  
> 63. Glean Pricing 2026 \- Plans, Costs, and What You Actually Pay \- Workativ, accessed July 21, 2026, [https://workativ.com/ai-agent/blog/glean-pricing](https://workativ.com/ai-agent/blog/glean-pricing)  
> 64. Slack Tools \- Glean Docs, accessed July 21, 2026, [https://docs.glean.com/tools/connector/slack](https://docs.glean.com/tools/connector/slack)

[image1]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEYAAAAaCAYAAAAKYioIAAACm0lEQVR4Xu2YuasUQRDGP2+8UUEERVSMxEgMFTUx00QUDQRFPDAwEcwMRMTIwD/gyUMxFdTASFRQjAxFjcQLTLwQDwSP+uhtt/m2umdm9719G8wPit3+qrq6p6dnunaBlpapYJYKI8hMFQZlkQrCJ7OlKo4of1Xolz9mF8x+qaPDbbMzKg4BXmDJHpvN+R/dZb3ZdhWbwsXgICc6nx6PVBgiV9Cdn8IFoG+zOhD0TSo2gQluqJhwzWyGikMk7o4cY/D9Z+HrtViL0Hm3OhL6Tj5BVC3MRuT91KerWIdzyCclq1D2R+aarU7aC5Pvg7AMYfzX6khYgRDjHR7UG70bmSTeidROp0EIL+TSwsS7dd7skNnxTntxEjMIlxHyHVZHwl6EmGnqMH6afVexDkyYO4nIO+QX5jl6fXGBJ4o6+X4gH/MQeV8RdhpXMaE0Mep3He2paHuk3YTS+BH676jY4Sqq+7uw01YVE+hnjePh9aW2T7RT0m5C1Y07gBCTq3ZzJ1YlVZ1ewo/ZBV/3tEFgvjUqJtBfGvMeyn4XHmNVnbhFvZgd6NX1BOPL8EjSbspB9I6RwpOK/tKL/hXKOVz2I/+YRE7CT+wtKk+AZ0n7ltmxpB2JF+wdrylv0TsGmY/uTuH3Eox5oGIVn1E+BiPe5EisgWjxyGRNQZZ0Pr2+/IX+Avlq+xu6eWm/EU5OLvxXhMe4Luy/QMUqvEl7fFAhg+aLxZnHOrOLKk4w3Jlc1Fqk2zM3aWWe2XUVhaPozffE7JLZF9EJ8/VVqjeA89mpYg4G873CC2FhVBe9aCVu/xQWh/z/Zrno5I0KkwAfu9rcR7iAj6JXsc3svYrGTYTnn2W3V4FukfawGFdhstmgwgiy0my2ii0tLS1TwT81a7gYcz04mAAAAABJRU5ErkJggg==>

[image2]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEQAAAAaCAYAAAAOl/o1AAACcUlEQVR4Xu2XS+hNURTGvzxKQkkpYyUGUgoTE0V5pJiQDAzISMwUEzEyUUw8Cn8mZIw8ChNGDJiYepSEogwQwvrse7J8d6997uP8B6fOr77u2d9ae9+z1z3n7r2Bjo42M0eNEZinRlvZZbqq5gjsNN1Us8TvQDNNrzK+ai76OYj+vEg5Fpi+qTkGz9QYhHtIN3hFA8YFxDf/Dim2SAPGcpQnTn+Gmojzx2HoMW8hdbokPikVhESTrivIXdNa8aaZzonXBLyH/WqW4HvGThc1gPqCbEKKPxG/riArTcfEe2SaLl4TXEN8H1muI3U4rwHUF4TkJh4VxLdPumuiuREr3PVidx3B/EHH/ktTBZnlvFxB+F9TGqsUm2L6arpjWt/7ZP4hn1SAuUvVjGiqIJud5wvyAmnl0AJ5liCO7UWKccyKBz1vUJh7QM2IpgriN0K5J4TxaKwNiGM6TuSdkraHuafVjLiB1IGTV4YpiCdXEKLtio2IY/S5Eqr3ULzn0vYwf0LNiHFWmd1Icd0ARQWJWIg4l/6qjLdOvBLMP6JmxG2kDpc1gPqCRJMetiAkys356u2RtsJ8buUH4j5Sh9z5oVSQz0ix+RrA5BaEr4r3uPqQ3I65gvlcqYp8N71FOre87H1+RP4s88v0w/TF9Mn0GHl4lvlgeo1/Y/I7BjmfvFGjR7UCUdy7vDcd/y8DeCptz1n0F7UVrDEdVjMDJzc740Xwx9yuZlsoTYzwsdecLUivje58KzS/VfA11kOf5wz6J7jPdMJ0VHzC3fMONduGTpisNv1EehL4P8acqS6+zF17cmO1kq1qjMA2NTo6Ojomiz+wYeaDedZB7AAAAABJRU5ErkJggg==>

[image3]: <data:image/png;base64,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>

[image4]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABMAAAAaCAYAAABVX2cEAAAA2klEQVR4XmNgGAWUgEdA/B+Kz6HJkQV8GSCGSaNLkANALgIZRhUA8yZVAMigpUh8fiA2ROITDbwZIIZpQPmPgbgRiE8DsQJUjGjwmwHhxe9Q+hJUjBPKJxqANP1hgBhKCIBcjBOAbAYZ9glKf0GVhgOQHMi1K9AlkEEDA0QhI5T/Doi74bKYAK9hXxlQk0QzEF+AsgsYMGMUr2Eggz4g8ZcDcQeU/RdJHAZwGgbyGsgwETRxkNhPNDEYWIUuQAlYgy5ACViLLkAOAJUq+QyQ3OEPxFyo0qNg+AAAqmExKfP2xUEAAAAASUVORK5CYII=>

[image5]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAkAAAAaCAYAAABl03YlAAAAi0lEQVR4XmNgGLzgJRD/h+IlaHIoIJEBoogJXQIZ3GGAKMILQAq+oguiA5CiOiBmBGIvNDkw8GWAKLoAxHuBOAjK10VW9BMqiAzmoYuBOFeRBYDgPFQcDEBeBnGi4dIQAAs3MHBD5iABFEVhyBw0MUmYAA9UABmA+JfQxBiUoRIg/ASImVGlRwHVAAC1XSVX0iee6AAAAABJRU5ErkJggg==>

[image6]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAFgAAAAaCAYAAAAzBZtTAAADFUlEQVR4Xu2YOYgVQRRFr/uOCi4IogYqaK4mgwYGKkYmmgtGghiogYKCiokOmA2DwQQyIii4pwYimChmLoGgoLihuKG4+y5VxX/9flX1D7rbz9AHHtN1X1V11+2ppT/Q0jJWGG+Flmr5a4UKmCIxZMUEP60w1sgZzFwuznaqFpiK3g1+Z4U+IIzvSkTL+RWlrMFLuDqLbULYDpe7b/Rp6M3gVxLjrNgnWIMDZX51Udag7K2FF6CNmo7eDM71+7/pG4OPweX3Km0Gyg3eKbHHin1EowbnzKKxrHNNaTORb0P+WMHDmbBJ4gTcTKiCFRIr/fVaOONiS94A3PJGGjV4oRUVF+HqjChtFsoN/mgFYRCuLw5yvr9+XKjRmVEhYvpsrx325YMSo/76qMQTiR++Dnnkc7slDkh89+XaDR5GPk/CoFYrjQPMGfwG8c0tLDcalm8bbYfXtyjtjMQ6VSa2r9MoGktGJF4YjbDtVSuiu89Scg2Ye2BFBT9SgsGaOcgbbOun4ItiXZ42LNT5n6bLFqsdj2gs84VZGjOYR7EUl+DqnDT6XOQN1huiZR9cn1yjObV5/bpQw3EPxWc/r64DzG9V5acSR1SZsM4ao5HaDV6KdI5wnWT+s00gbzD7TfEJrk9dh2UuKRbOEuZ4Hy43sSWHecYFiff+2kJtmxXhdL1xB2J9ZEk14FdaKkfCw8fIGfzQCh7u6uzvptGpvfXXnDGa8AznjB647v/GzA+wvZ2BhPoNKyI95iSpBjkD76C4/llyBqf6XA6X424f4PGK2geJCeg28hDyz0l9CfLHvQ2It6dmN1cSq5tFN5gs8c1rIX7D7bzU+bsB18UyUgbToFVWVMyT+ILOvXnc4xGRzxD7byKsp08wGj63HksI+wMTz8Y6v9mUyVeJ5xLP4L5e7ctOEjqokpTBddwrxS6J/VYUFsA9x0abqIs6Bh0zeBHim0Zd3JKYZEUPx5w7yVRKUwbfhfsZsyl4Rv+F7nteRj1jTlLHzWIG13GfMtbDfZLr9fRUoUYDLLNCRUwsKbe0tLS0tNTCPwjhAALgbA4aAAAAAElFTkSuQmCC>

[image7]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAmwAAABmCAYAAAB2kc1qAAAO/klEQVR4Xu3dC7Bu5RzH8b8kpKKQyuUcoZJrRCHZLpEyiNyjQxSmXBJymRxKJBqRW9N0JhOmIZcoqUxCgxCaOrl2dnRxp6SUXNavtZ72s//v86zLu9/9Xr+fmWfevf7PWu9a63332et/1nouZgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAdHP7ovzPB3GL432goycVZT0fxMDwewsAmBm66N3WBxP2KcqqouxblKcsrrrFi4qyh5VJyl6ubpheX5RTinKBr3B+Y/XnPu8DfSKpWD4bFeUYHwQAYNr82wca/M3KBCSXhPyuKL/wwRHQ8R3tgxHVK+nU6wmuThR/qA8uQe7zWk7a57lFuZ2VyedfFtWmbW8L329cHhWvNGb2K8offRAAgGlxRlEu9sEGuni/t3pN3Zm62QdGRMd3Fx+svNIWEqhD4orKLjb4BOuKopztgy39yAdaUOL8XxfTOX3cxbztbHGidrmVCd+4+3NRjvBBAAAm3eOsv6TksupV26YSkH7ecznUHcc1Vl+vu47v9MEl2tLq91nntz7Qgvb1Hhc7v4rXuX9RvueDE0LntpUPAgAwyXRxW98HW7hX9RruUm0c1enO1InR8rDtXr1+pig/iyscHfeVPlhZYc1JjWzmAy3ofQ/1wRau9oEWtK+nutj7q3id+9rkJmz3sebzAwBgYqwsyiU+2MLD3bIujpdGy+cU5d7R8rCcVZT/FGU3K49J5bmL1ijdYAv1ofjHhk1JzSNtIfFSW72Dq+U2/mXlMXT1Bx9oQce0s4u9vYrXWWllwnasLXxGT45XGHP/LMoGPggAwCS6ycqhPLr6slsOF/R4ediUMMb7faxbTlF9rkPBWstvv5OVdU+PYv4zqKMkue26sX4TNiWXsTdX8Tr3tHIdtWWTTaplJXuTQEnqvA8CADBpdAes6aKd47cLyc1R1bKvX26hHZ6GEwn0SK/pOOrqr7d8veK6S+Zj/i5dztcs/951BpWwvaWK17lNUbZ1sRutebtxomPt53E/AABjQxczPT7sSoO/atwyT+8XLuZx4lTnAUV5Y8uS6+kp8b6D+UQstqPV19d9PqpTkhisrGLPiWI/KcqrouXYGqvft/jzV/lHIqaydbVNivazq4utruJdnWn9bTcqOta2STQAAGNJF7N+hj/Q8BerfLDwdSvf8zRfMQTar9os+VhdcqFhLXRnMGedpbd/pvXG2yRgsfOs2/pBv3fYXuBin6ridVKfn9om+tg404DJk3S8AAAsoqEqdCHrZ1wt3eVJ0SO01EV+GLTPwxKxTxTl0UU50NWJEjzdncoJCaj3ROuN+/PWQLx1w0poeA7/Hm30m7CpA0Xsh1W8jj8n6fe4RyWMoxe3NQQADJHGiFoKdfufZamLcRMN23GqldvNLa66le5adX3fQdA+fx8tqzOFYiuq15RcPFD7rdw6cfyl1fLHquUwYHBuW1HdB3ywhX4SNt9e7U7V8iOi2GuqWJzYnlyUz0bLe1q5zluj2CTQMdd9FwCAZaKxtTT211Kowf2ffHCG6ALWdRDWa62c0kgjyevnnHkfGBJNSaTzUsN4+Ua17BvOS7gb2ETr+Ab78m5bSAT2rl43r+qUEOmxqe+UENP6Wq+rfhI20Z1G7fMH1asfl01+7ANW9gYO56ni28JNAhI2ADMj/oMdioZQWJmIx+Xnlh5UNAwp0FS+HTaIaEDUX/pgn/SYKLWPYYnPdZg03IH2eaSvmCEvsXafuxJUtTdr4t9Ld/j2cLFAd+RynRma9JuwTRMNL9LlEWdIUgFgJmiuyFxyoR6Dir/BxddU8e+6eBDez4+Dpf/9K56aizK1/6UY9Pt11WassEE73Mp9PsxXzADNq6lHhLqIt0mawuPDOvtb7zph+ZpF0ZLqUne42tjGB2aE7ohqsOYwd+2HFlfX6jKgMQBMhZBgebqTpvgqFw9y24W4hnZI8ds8yMo7I4O0YVHO9cEh8+e53NRpYNj7HBc6b3VC6HL+21v6Pw+vLcpVVr6Xho7QbAfBdZa+G3aRlY9QZ9nLLd8Gso2uCdv9rNv3DQATL5d4hYRNj3pS1BMvtV14v1wHAr+NH7ZhUPx+hm3Y+899j7NAd2r6SfqVZGjbmF+O51LN+aoPzCC1P53zwQ66Jmyibdp8PwAwFXIX+pCw5S6EGjpC9S9z8VTCFvecOz76WVL79h5oZRsXafsHWu/bz/AWg5I7Lz0qVrIbzidlB1toFP/4onzT0o3sY7nvEfWW+gh51nsmBwfYaBK21JyyADCVchf6kLC92FdEUtuGmB5RrSjKQdVySph6KEfDTayOllP7y9F6H/XBhCd0LG3541SbwL9Hy3e1ch2NKRWcZIsbw6tej36U4MVDMaRoXb9PYFg0hMicD3ag391+Ejb17AWAmZC70C81YVOvz3XRckqqYXegtkS+7itFudDFcrTt2T6YoP2kyqutvGugaYj2K8orLN+eL8Ufu5Y3dbHTq3jgt3mftZ+Cp+5zBpbbIBK2Y3ywgbb5sA8CwLTKXejbJmzxwKYhphI/El0X/RzTEBSpfUvquLS8l1veJ1qOqa7rmGSDFB97roG0phaK436dMHNBG6nPq456NFIouVJnp0RRswclbT6+XbVNE/3udk2+tM0nfRAAplXuQt+UsKkNlup9p4TwfnHClntscaSl963G4Ir7O2R+3S3cckzratDVUYmP9R1uOdCYXnFcj0zjicb1GFWfURu57zFHA+VSKLlSx09Sr/IFK2eF8PFcG1hPv7sf8cEG2kbzpwLATMhd6EPClruDlRtGIrxfUyN5CdPieL+yMh53Gti3igVbWjmOXI7WPcsHE8Lxti1txevu6JaDN9niuEalf5uVn63mhWx7sZOuxwcM0iAeiR7ngw20zQd9EACmVe5CHxI23wtUFFNdKiEK76fx1Zpsbel9f9p641dGMfWi1JyJfp2Y6g7xwSHyx6ble7iYxviK1/PbdJH7HoFhGETCpjlruxj1v3EAGJpw5yd1odecnIqvjmJ3Lsr1VVwjjXthLkcVNdpvI7XvMGSIhvOQ71fLYd0wr6SSuByt23YIkEHTUBHa/x2i2NoqFoSBXp8RxTR7RDhPFU2hlEqYU+LPZ5rp0VtujL82NK1abpYO9G8pCdv6Vv7unm+94+DV0TbP90EAwPJQO63cH2m1UdOE7uFOXPzHualHWd0k3aOkZOPZRVnPxUOyu8LKC5iWlVyoU4fiTWPKjXPCFiehu7m6mH4X1IZP6/ketaJppwaRhIcEYRQ0bpuGm9Hcu22l1m3btnFYlpKw9UP/Nkb1HQLATNIF+DQfdFKPSMNyqtHxs2xh4NlJ8XnrPcdA8Xf5oHOx5bcfB5oNoO74wph811avfkBaJbhfdLGlUNun1DRTy0n/AdHMHuqwo591nrn/rMTUeSZOelX2X7TG6GnoG82hOyzhLjYAYIia/vCGi5SP6fFYil93EugORe64FX+wDzpHWble/Ih1nKS+w5gmcFe9BhJO3T06xwcGoO54mjR9Hyl+f0oY1cGkSbjLGgpDWZQ9z/3nCQBYZpquKtWBYSNbuOOi8uvF1Unq1aoelpPoBCvbDu5clLsX5WlWnrd6jjbZ1cp1P+crxoSO7ac+GFG92kfmLMfFWZ0+/KPptuZ8oIEeg/tz0ONhxep6O8sVPoBbE3wAwJCprZr+CC+FHp3mxo2bBSGxHRd6zLnCytkidFyajitFnTNUr/VS1ljzed2xKI+JlreJfs7Zvihn+mBLcz7QQO3v/DmE8266e3i5D2DsftcBAGhtXC5iR1t5HC+0cn7I0AYrJRxzXPwdJyU76pCQozaQGnQ13LFS2X3RGnm542oy5wMNct+NYk130OZtYao2FbVXnHX6HJoG+AUAYCyFdmxtGrIvl9ADMz4GLdfNifodSyczgerO8MHKZbZ4WzV8r3svr8u6sTkfaFCXsKXiMQ0kvXe0vMbKThyzSlNe6TN7nq8AAGBS6EKmQY9HRfvXo1AfixMOrylpUd3hPmgLPWPjIU9WWW9yqAQnp26/ot7Gfn5MldS8mSo5uXNU7CYfbEHbjVtP0WFJPV4GAGCi6OI/qouZkrLUvlOxmOpTnU4C1a/2QUsnQfPWOydlU2eHOgdZ7/yYKql5M1VyUscaxt67yMXb0Hbn+uCM0Llf4oMAAEySh1hvYjAsakvm970iEYttbmV93Yj1qj/JxTTrhuIa1yym2N1crE7dsdWZ84EG89a7L82Hq9iJLh4L7fI8xb7lgzNC564e5AAATLRRXdDOs97kYk1RLq1+1lRb3qFWblPX7k71aucWC23lDnNxv/8D3bLn129rzgcazFnvvvasYurhmnOE9W4niq3ywRmgBPavPggAwKRKXeSXW5gDNtAcsHpEu6+Vw7acHNUFam/WdKwaWy61jmIaVFbCyPfxeqdYeQcvNwK/jukCH2xpzgdauKooO0TLOtZTo+UQ8+fql/U+uU4Y085/FgAATDRd2PbwwSHY1haSDj2eFf2cG9Ffdet80Flp+Qt1GDLkkOrVr3ehW44da70dJNqa84GWdHxrrRxz0D/mFc0z6mcy0OC+2k6Pf8PrLNLMJv77BQBgomkA4Um4uOkY63qQBm3OReuEx69xLOdmH+hgzgewrDaw8rvUnVQAAKZKuNs1bjTFmI5LsxG0TZo2LMpxPhh5nZXvuVUU05RnX7LyEaJ3cFFu9EGMLX23G/sgAADTQknLZj44YtdZORisHv916Ryhi3Zq7k8Ni6E6lbhB+iZFucHKZM8bx0QWaVcX5QAfBABg2oxjcqJBX3fxwQZKwFJ35OKprPy0VilKGLsM/YHROt0HAACYRltYOtGZRJtaOWl6vzSEhnqHYjKM4382AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAz4f9dWpfceD9mDQAAAABJRU5ErkJggg==>

[image8]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAADYAAAAaCAYAAAD8K6+QAAACfklEQVR4Xu2XTchNQRjH/8h3CEXKQmHteyFSLKQsFaX0LixEFkJZKYn0lhU7ZWVFSrGQj7K0wM5CwkISiSTkM56/54w7939n5p774S11fvX03vk9z5lzz8ycmfcCDQ3/kqkqxoKlKoRHKvpgmsUXlb2y0+JXFDnmwPO7q78pTlpcUNknuyweq+wHftkrKiOYPwwfydSDnbV4o3JALls8VdkL6+BfdqUmIpjnrOVgfoHKIcB+J6msyy2kZyGmlN+Ccn4Q2O95lSUmWCysPnd7v+ahnH9h8U1lgjXSnijtFDdRvncbby0eWhy3eA2/8HpbhXMOrYeOQ6HjO5ZjhcU7+My+shiBXzMuLsqwFel7trEMXrQvcqOVK43eD4s9KiN4PXfXFE/Q+cVyA5RiLmrUpjr8kHAK8zxbcjC/ViV8qTN3VTxdvEJ4NnJmcrB+pspAeE/ei6f7JE6p8+CrVRrP4blVkWOdDgS3dJ6POVg/X2WAZxALDkVufOWORk7haNZ5sO0q4V6vvZZwJThTxfoT8ALuhoGDlZtcte9FucApdOkYnj+iEukHU7cB5fNxOTr7aGMxvCBs8SS+yTGLJa3UX8KuWYL5GyqNZ2i/dqRq344cB7rU/wGU83+YZfETXsjtnvD8YXt/KBKYu69S2Iv8zcOMP4C/BvwcVshsi20WH6t2CtZfUjkM2PFGlQlYN0WlwI1LB+C7xSZxMVo/EOyMS3BH9bkOd9BaBTnY19eEIzx2lNPwfyaGBm921+IlfCnVpTQIYXc9I56vxmb4a6LUWQU98Rne6UVNdGEROmeE8J9s/tzhecl8PAAzLKZH7QB/i61X+b9T+vnU0NAwhvwG4y2tKw+sKpEAAAAASUVORK5CYII=>

[image9]: <data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAABMAAAAaCAYAAABVX2cEAAAAzElEQVR4XmNgGAWUAhEg/o+Ep6NKg8FXBlQ1IIwXLGVAKGREkwOBuUAchi6IC4AMWQKl16HJgcBfdAF8AOZ0XN7AJoYVyALxMij7HQNEYzhCGgwuoPFxgtlArAFlKzNgd10gGh8nQNd4DyqWDeXbIskRBG/Q+MIMqK7biySHF4CiuxBdkAFhmBSUJgpcA2JmdEEgiGWAGPIcShMF8CmEuW4qugQ2IMmA37CNDBB5cXQJZDABiD8yQAL+LRB/A+IIFBUI8B5dYBSMgiENAOmINsXBtmLOAAAAAElFTkSuQmCC>