import "@typespec/http";

import "./models/common.tsp";
import "./models/user.tsp";
import "./models/problem.tsp";
import "./models/contest.tsp";

import "./api/contest.tsp";
import "./api/service-admin.tsp";
import "./api/user.tsp";

@service(#{ title: "traOJudge" })
namespace traOJudge;
