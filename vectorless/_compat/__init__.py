"""Framework integrations — optional, loaded on demand."""


def get_langchain_retriever():
    """Get the LangChain VectorlessRetriever class.

    Raises:
        ImportError: If langchain-core is not installed.
    """
    try:
        from vectorless._compat.langchain import VectorlessRetriever

        return VectorlessRetriever
    except ImportError:
        raise ImportError(
            "LangChain integration requires langchain-core. "
            "Install with: pip install vectorless[langchain]"
        )


def get_llamaindex_retriever():
    """Get the LlamaIndex VectorlessRetriever class.

    Raises:
        ImportError: If llama-index-core is not installed.
    """
    try:
        from vectorless._compat.llamaindex import VectorlessRetriever

        return VectorlessRetriever
    except ImportError:
        raise ImportError(
            "LlamaIndex integration requires llama-index-core. "
            "Install with: pip install vectorless[llamaindex]"
        )
