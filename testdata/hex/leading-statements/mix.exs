defmodule Demo.MixProject do
  use Mix.Project

  def project do
    [
      app: :demo,
      version: "0.1.0",
      elixir: "~> 1.15",
      deps: deps()
    ]
  end

  defp deps do
    IO.puts("Note: {:fake, \"~> 1.0\"} is deprecated, see https://example.com")
    # Pinned below the next major until the API migration lands.
    deps = [
      {:jason, "~> 1.4"},
      {:ecto, ">= 3.0.0", only: :test}
    ]

    if Mix.env() == :dev, do: deps ++ [{:credo, "~> 1.7"}], else: deps
  end
end
