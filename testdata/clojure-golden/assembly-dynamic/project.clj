(def netty-version "4.1.135.Final")

(defproject org.example/dynamic-demo (or (System/getenv "PROJECT_VERSION") "2.0.0")
  :dependencies [[org.clojure/clojure "1.11.1"]
                 [io.netty/netty-transport ~netty-version]])
