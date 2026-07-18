defmodule AppTwo.MixProject do
  use Mix.Project

  def project do
    [app: :app_two, version: "0.2.0", deps: deps()]
  end

  defp deps do
    [{:ecto, ">= 3.0.0"}]
  end
end
