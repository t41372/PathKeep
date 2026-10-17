# Decision models (2026-10-17)

A new kind of model appeared in September and October 2026: it reads text (some also images) and answers questions whose possible answers you define in advance, with a probability for each answer. It writes no text. This note records what exists, what runs on this machine, and where it could help PathKeep. Nothing here is built yet; most figures are vendor-reported.

## What they do

A request carries one piece of evidence (the "state" or "input") and up to dozens of independent questions about it. Each question is one of three types:

- yes/no: the probability that a statement is true (`noul` in Jev, `predicate` in OpenAI);
- choice: one option from a fixed list, with a probability for every option;
- score: a level on an ordered rubric, returned as the probability-weighted level.

The answer is always one of the declared options, so software can act on it without parsing. One forward pass answers a question, input tokens are the only cost, and vendors claim roughly ten times the speed of a chat model on the same job. A decision model cannot name, summarise or explain anything.

## What exists

- TypeSafe Jev: hosted only, waitlist, announced September 2026. Endpoint `POST /v1/systemone` with `state` and `questions` keyed by id. Reported at $0.042 per million input tokens.
- OpenAI Decisions API: `POST /v1/decisions`, model `gpt-6-luna`, public beta since 6 October 2026. `input` plus `questions` (`predicate`, `choice` with `choices`, `score` with `levels`); answers carry `probability`, or `choice` / `score` with `probabilities` and `confidence`, or a `refusal`. $0.10 per million input tokens, no output charge. Limits on questions and options are not documented.
- Perplexity pplx-decider: open weights, Apache 2.0, fine-tuned from Qwen3.8-27B, 262k context; v1.1 on 6 October 2026. Community GGUF builds of the 27B are 15–25 GB; a community "flash" build is about 5B.
- OpenJev: about 27B, CC BY-NC 4.0 (non-commercial). Scores each option's letter at the first output position, then calibrates. Up to 52 options per pass, prompts up to 16k tokens; GGUF and MLX are text only.
- Decision 1.0 (vLLM Semantic Router): 0.6B to 9B, Apache 2.0, most on Qwen3.5 backbones, 16k tokens of input (1k for the two 0.6B encoders).
- "LLM-as-Jev" (arXiv 2610.02076): an ordinary chat model does the same job by reading the probabilities of the option labels after a fixed prompt. Qwen3.5-4B with no training scores 81.4 % on the public JevBench items, level with community models fine-tuned on the same backbone; LoRA tuning adds 2.6 points.

## On this machine

LM Studio 0.4.26 (installed: 0.4.26+4) serves both formats, `/v1/decisions` and `/v1/systemone`, but only for a model whose type is "decision" (`lms ls --decision`; the REST `type` field). A request naming the installed `qwen3.8-27b-splash` fails with "There are no downloaded decision models." No decision model is downloaded.

The bundled Splash engine already scores options this way for any chat model (`server/judgments.py`, the SemIf direct-options prompt with thinking off): final-position logits restricted to the answer slots, at most 64 questions and 255 options per request. It labels the result "uncalibrated as decision confidence". LM Studio does not route `/v1/systemone` to it for a chat model.

## Compared with what PathKeep has

An embedding gives one vector per page that every later question reuses; it measures likeness, not a judgment. A decision model answers a stated criterion but costs one forward pass per page and question set. OpenJev reports about 80 ms for a short text decision on an H100; nothing is measured on a Mac or on the target machine (4 cores, 8 GB). At 100 ms a decision, one million pages is about 28 hours, so a decision per page of the whole history is a long background job, while a decision per topic (hundreds to a thousand a month) takes minutes.

## Where it could help

1. Categories that stay the same every month. Classify each month's topics, or pages, into a fixed, user-editable list (work, study, entertainment, shopping, admin, news …). The categories are identical across months, so a per-month chart compares month to month and nothing needs a generated name, which is what the topic card lacked ([2026-10-topics-of-a-period.md](2026-10-topics-of-a-period.md)). Today's "kind of site" card is the no-AI baseline: it classifies by site, a decision model would classify by what the page is about (a lecture on a video site is study).
2. A filter tool for Ask. The agent states a condition; the host asks it as a yes/no question of every candidate page from `search_history` and returns those above a threshold with their probabilities. The chat model then reads a short, precise list instead of hundreds of titles. The same question reranks semantic search results.
3. A local check before Ask sends pages to a cloud model: flag pages that look sensitive (health, money, sign-in) and hold them back. Needs a product decision on what counts and what happens.
4. Splitting wrong topic merges ("are these pages about one thing?"), like the API proxy merged with the chat site it is named after.

## Before building

- Accuracy on our input (a title, a URL, a site; Chinese and English mixed), on a small labelled set.
- Speed on this Mac and on the target machine. Only the smallest models fit beside the app in 8 GB.
- A third provider kind next to chat and embedding, speaking the OpenAI format (OpenAI, LM Studio, LiteLLM) and the Jev format (TypeSafe, LM Studio). Whether to fall back to option probabilities from a chat model's `top_logprobs`; a third-party report says LM Studio returns them for GGUF and not for MLX.
- Measuring needs a decision model downloaded in LM Studio (3–25 GB); disk is tight, so ask first.

## Sources

- OpenAI, [Decisions guide](https://developers.openai.com/api/docs/guides/decisions)
- TypeSafe, [Introduction](https://docs.typesafe.ai/introduction); InfoQ, [TypeSafe AI releases Jev](https://www.infoq.com/news/2026/10/typesafe-ai-jev-released/)
- LM Studio, [Decisions](https://lmstudio.ai/docs/developer/openai-compat/decisions), [Decisions (System One)](https://lmstudio.ai/docs/developer/jev-compat/systemone), [0.4.26 changelog](https://lmstudio.ai/changelog/lmstudio/lmstudio-v0.4.26)
- [perplexity-ai/pplx-decider-v1.1-27b](https://huggingface.co/perplexity-ai/pplx-decider-v1.1-27b), [openjev/openjev](https://huggingface.co/openjev/openjev), [Decision 1.0](https://vllm-sr.ai/blog/decision-models/)
- [LLM-as-Jev](https://huggingface.co/papers/2610.02076)
- [nowledge-mem issue #128](https://github.com/nowledge-co/nowledge-mem/issues/128) (logprobs from LM Studio GGUF vs MLX)
