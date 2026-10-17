const { size } = require("./first");

module.exports.fuzz = function (data) {
  size(data.toString());
};
