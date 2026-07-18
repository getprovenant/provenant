defmodule AppOne.MixProject do
  use Mix.Project

  def project do
    [app: :app_one, version: "0.1.0", deps: deps()]
  end

  defp deps do
    [
      {:phoenix, "~> 1.7.0"},
      {:app_two, in_umbrella: true}
    ]
  end
end
