---
license: apache-2.0
library_name: transformers
pipeline_tag: text-classification
tags:
  - text-classification
base_model:
  - bert-base-uncased
datasets:
  - imdb
---

# No-Identity Demo

Synthetic Hugging Face repository fixture whose files carry no `_name_or_path`
or `model_name`, so no identity PURL is derivable. The card and config still
merge into one package reporting license, tags, and architecture.
