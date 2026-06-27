defmodule MyApp.MixProject do
  use Mix.Project

  @version "1.2.3"

  def project do
    [
      app: :my_app,
      version: @version,
      elixir: "~> 1.14",
      start_permanent: Mix.env() == :prod,
      deps: deps()
    ]
  end

  # Run "mix help compile.app" to learn about applications.
  def application do
    [
      extra_applications: [:logger]
    ]
  end

  # Run "mix help deps" to learn about dependencies.
  defp deps do
    [
      {:phoenix, "~> 1.7.0"},
      {:ecto, ">= 3.0.0", only: :test},
      {:jason, "~> 1.4", optional: true},
      {:credo, "~> 1.7", only: [:dev, :test], runtime: false},
      {:plug_cowboy, github: "elixir-plug/plug_cowboy"},
      {:dynamic_dep, System.get_env("DEP_VERSION")}
    ]
  end
end
