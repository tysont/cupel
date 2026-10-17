const { firstCode } = require("./first");

module.exports.fuzz = function (data) {
  firstCode(data.toString());
};
